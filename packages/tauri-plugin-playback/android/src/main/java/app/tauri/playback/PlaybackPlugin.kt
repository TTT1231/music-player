package app.tauri.playback

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import androidx.activity.result.ActivityResult
import app.tauri.Logger
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class UriArgs {
    lateinit var uri: String
}

@InvokeArg
class StateArgs {
    var playing: Boolean = false
    var title: String? = null
    var artist: String? = null
    var durationMs: Long? = null
    var positionMs: Long = 0
    var rate: Float = 1f
}

/** 原生事件（焦点丢失、耳机拔出、通知按钮）统一从这条通道发回 Rust/前端。 */
object PlaybackEvents {
    const val EVENT = "playbackEvent"
    val listeners = mutableListOf<(JSObject) -> Unit>()

    fun emit(kind: String, vararg pairs: Pair<String, String>) {
        val data = JSObject()
        data.put("kind", kind)
        for ((key, value) in pairs) data.put(key, value)
        synchronized(listeners) { listeners.toList() }.forEach { it(data) }
    }
}

@TauriPlugin(
    permissions = [
        Permission(strings = ["android.permission.POST_NOTIFICATIONS"], alias = "postNotifications")
    ]
)
class PlaybackPlugin(private val activity: Activity) : Plugin(activity) {
    /** 通知权限只问一次，拒绝后不再反复弹窗；被拒时通知不显示，播放不受影响。 */
    private var askedNotifications = false

    companion object {
        var instance: PlaybackPlugin? = null
        private const val REQUEST_NOTIFICATIONS = 1001
    }

    init {
        instance = this
        PlaybackEvents.listeners.add { data ->
            try {
                trigger(PlaybackEvents.EVENT, data)
            } catch (e: Exception) {
                Logger.error("转发播放事件失败", e)
            }
        }
    }

    /** 选择歌曲和歌词（可多选）。返回每个文件的 URI 和显示名，并申请持久读取授权。 */
    @Command
    fun pickFiles(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
            putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
        }
        // 必须经插件启动，结果才会回到 @ActivityCallback
        startActivityForResult(invoke, intent, "onPickFilesResult")
    }

    /** 选择文件夹。返回其中（含子文件夹）全部文件的 URI 和文件名。 */
    @Command
    fun pickFolder(invoke: Invoke) {
        startActivityForResult(invoke, Intent(Intent.ACTION_OPEN_DOCUMENT_TREE), "onPickFolderResult")
    }

    @ActivityCallback
    fun onPickFilesResult(invoke: Invoke, result: ActivityResult) {
        val data = result.data
        if (result.resultCode != Activity.RESULT_OK || data == null) {
            invoke.resolve(files(JSArray()))
            return
        }
        // 多选时结果在 clipData，单选时在 data
        val uris = mutableListOf<Uri>()
        data.clipData?.let { clip -> for (i in 0 until clip.itemCount) clip.getItemAt(i).uri?.let(uris::add) }
        if (uris.isEmpty()) data.data?.let(uris::add)
        val out = JSArray()
        for (uri in uris) {
            persist(uri)
            out.put(entry(uri, displayName(uri)))
        }
        invoke.resolve(files(out))
    }

    @ActivityCallback
    fun onPickFolderResult(invoke: Invoke, result: ActivityResult) {
        val tree = if (result.resultCode == Activity.RESULT_OK) result.data?.data else null
        if (tree == null) {
            invoke.resolve(files(JSArray()))
            return
        }
        persist(tree)
        // 大文件夹遍历可能较慢，放到后台线程，避免卡住界面
        Thread {
            try {
                val out = JSArray()
                walkTree(tree, DocumentsContract.getTreeDocumentId(tree), out)
                invoke.resolve(files(out))
            } catch (e: Exception) {
                invoke.reject(e.message ?: "读取文件夹失败", e)
            }
        }.start()
    }

    /** 按 URI 打开文件并返回原始文件描述符（Rust 侧负责关闭）。 */
    @Command
    fun openFd(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(UriArgs::class.java)
            activity.contentResolver.openFileDescriptor(Uri.parse(args.uri), "r")?.use {
                val response = JSObject()
                response.put("fd", it.detachFd())
                invoke.resolve(response)
            } ?: invoke.reject("无法打开文件")
        } catch (e: Exception) {
            invoke.reject(e.message ?: "打开文件失败", e)
        }
    }

    /** 注册原生事件通道：参数结构（event + handler）与基类 registerListener 一致，直接复用。 */
    @Command
    fun registerPlaybackListener(invoke: Invoke) {
        super.registerListener(invoke)
    }

    /** 退到后台而不结束 Activity：结束 Activity 会让 Tauri 退出整个进程，音乐随之停止。 */
    @Command
    fun moveToBackground(invoke: Invoke) {
        activity.moveTaskToBack(true)
        invoke.resolve()
    }

    /** 把播放状态同步到前台服务（MediaSession 通知）。 */
    @Command
    fun updateState(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(StateArgs::class.java)
            if (Build.VERSION.SDK_INT >= 33 && !askedNotifications &&
                activity.checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
            ) {
                askedNotifications = true
                activity.requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), REQUEST_NOTIFICATIONS)
            }
            val intent = Intent(activity, PlaybackService::class.java).apply {
                putExtra(PlaybackService.EXTRA_PLAYING, args.playing)
                putExtra(PlaybackService.EXTRA_TITLE, args.title)
                putExtra(PlaybackService.EXTRA_ARTIST, args.artist)
                putExtra(PlaybackService.EXTRA_DURATION, args.durationMs ?: -1L)
                putExtra(PlaybackService.EXTRA_POSITION, args.positionMs)
                putExtra(PlaybackService.EXTRA_RATE, args.rate)
            }
            if (Build.VERSION.SDK_INT >= 26) {
                activity.startForegroundService(intent)
            } else {
                activity.startService(intent)
            }
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "更新播放状态失败", e)
        }
    }

    /** 持久授权：重启应用后仍能按 URI 打开。 */
    private fun persist(uri: Uri) {
        try {
            activity.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        } catch (e: Exception) {
            Logger.error("持久化 URI 授权失败：$uri", e)
        }
    }

    /** 文件的显示名（含扩展名）；内容 URI 本身不含文件名。 */
    private fun displayName(uri: Uri): String {
        activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0)?.let { return it }
        }
        return uri.lastPathSegment ?: uri.toString()
    }

    private fun entry(uri: Uri, name: String) = JSObject().apply {
        put("uri", uri.toString())
        put("name", name)
    }

    private fun files(array: JSArray) = JSObject().apply { put("files", array) }

    /** 递归展开文档树；根节点的 id 来自 getTreeDocumentId，子节点来自查询结果。 */
    private fun walkTree(tree: Uri, parentId: String, out: JSArray) {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parentId)
        val projection = arrayOf(
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
        )
        activity.contentResolver.query(children, projection, null, null, null)?.use { cursor ->
            while (cursor.moveToNext()) {
                val name = cursor.getString(0) ?: continue
                val documentId = cursor.getString(1) ?: continue
                if (cursor.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR) {
                    walkTree(tree, documentId, out)
                } else {
                    out.put(entry(DocumentsContract.buildDocumentUriUsingTree(tree, documentId), name))
                }
            }
        }
    }
}
