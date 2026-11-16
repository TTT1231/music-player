package app.tauri.playback

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Build
import android.support.v4.media.MediaMetadataCompat
import android.support.v4.media.session.MediaSessionCompat
import android.support.v4.media.session.PlaybackStateCompat
import androidx.core.app.NotificationCompat

/** 前台播放服务：MediaSession 通知栏控制、音频焦点丢失与耳机拔出暂停。 */
class PlaybackService : Service() {
    private var session: MediaSessionCompat? = null
    private var audioManager: AudioManager? = null
    /** API 26+ 为 AudioFocusRequest，更低版本为监听器本身；用 Any 避免在旧系统上加载新类。 */
    private var focusHandle: Any? = null
    private var noisyReceiver: BroadcastReceiver? = null
    private var playing = false
    /** 因短暂失去焦点（语音消息、来电）而暂停：保留焦点申请，焦点回来时自动继续 */
    private var resumeOnGain = false
    private var title: String? = null
    private var artist: String? = null
    /** 时长（毫秒），未知时为 -1；锁屏卡片据此显示总时长和进度条 */
    private var durationMs = -1L
    private var positionMs = 0L
    private var rate = 1f

    override fun onCreate() {
        super.onCreate()
        audioManager = getSystemService(Context.AUDIO_SERVICE) as AudioManager
        val mediaSession = MediaSessionCompat(this, "music-playback")
        mediaSession.setCallback(object : MediaSessionCompat.Callback() {
            override fun onPlay() = sendCommand("toggle")
            override fun onPause() = sendCommand("toggle")
            override fun onSkipToNext() = sendCommand("next")
            override fun onSkipToPrevious() = sendCommand("prev")
        })
        // 激活后锁屏、耳机线控和蓝牙的媒体按键才会送到这里
        mediaSession.isActive = true
        session = mediaSession
        val filter = IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY)
        noisyReceiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context?, intent: Intent?) {
                // 耳机拔出：立刻暂停，避免外放打扰
                if (playing) PlaybackEvents.emit("pause", "reason" to "noisy")
            }
        }
        registerReceiver(noisyReceiver, filter)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action != null) {
            when (intent.action) {
                ACTION_TOGGLE -> if (playing) PlaybackEvents.emit("pause", "reason" to "notification") else PlaybackEvents.emit("command", "action" to "toggle")
                ACTION_NEXT -> PlaybackEvents.emit("command", "action" to "next")
                ACTION_PREV -> PlaybackEvents.emit("command", "action" to "prev")
            }
        }
        intent?.let {
            playing = it.getBooleanExtra(EXTRA_PLAYING, playing)
            it.getStringExtra(EXTRA_TITLE)?.let { t -> title = t }
            it.getStringExtra(EXTRA_ARTIST)?.let { a -> artist = a }
            if (it.hasExtra(EXTRA_POSITION)) {
                durationMs = it.getLongExtra(EXTRA_DURATION, -1L)
                positionMs = it.getLongExtra(EXTRA_POSITION, 0L)
                rate = it.getFloatExtra(EXTRA_RATE, 1f)
            }
        }
        updateFocus()
        updateSession()
        startInForeground(buildNotification())
        // START_NOT_STICKY：系统回收后不自动重建（状态由应用恢复）
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        noisyReceiver?.let { unregisterReceiver(it) }
        noisyReceiver = null
        abandonFocus()
        session?.release()
        session = null
        super.onDestroy()
    }

    override fun onBind(intent: Intent?) = null

    private fun sendCommand(action: String) {
        PlaybackEvents.emit("command", "action" to action)
    }

    // 音频焦点：
    // - 可降低音量（通知提示音）：继续播放，安卓 8 起系统自动压低音量
    // - 短暂失去（来电、语音消息）：暂停，焦点回来时自动继续
    // - 永久失去（其他音乐应用开始播放）：暂停并放弃焦点
    private val focusListener = AudioManager.OnAudioFocusChangeListener { change ->
        when (change) {
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> if (playing) {
                resumeOnGain = true
                PlaybackEvents.emit("pause", "reason" to "focus")
            }
            AudioManager.AUDIOFOCUS_LOSS -> {
                resumeOnGain = false
                if (playing) PlaybackEvents.emit("pause", "reason" to "focus") else abandonFocus()
            }
            AudioManager.AUDIOFOCUS_GAIN -> if (resumeOnGain) {
                resumeOnGain = false
                if (!playing) PlaybackEvents.emit("command", "action" to "toggle")
            }
        }
    }

    private fun updateFocus() {
        val manager = audioManager ?: return
        // 用户在等待焦点期间自己继续播放：不再自动继续
        if (playing) resumeOnGain = false
        if (playing && focusHandle == null) {
            if (Build.VERSION.SDK_INT >= 26) {
                val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                    .setAudioAttributes(
                        AudioAttributes.Builder()
                            .setUsage(AudioAttributes.USAGE_MEDIA)
                            .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                            .build(),
                    )
                    .setOnAudioFocusChangeListener(focusListener)
                    .build()
                manager.requestAudioFocus(request)
                focusHandle = request
            } else {
                @Suppress("DEPRECATION")
                manager.requestAudioFocus(focusListener, AudioManager.STREAM_MUSIC, AudioManager.AUDIOFOCUS_GAIN)
                focusHandle = focusListener
            }
        } else if (!playing && !resumeOnGain) {
            // 短暂失去焦点时保留申请，否则收不到焦点回来的通知
            abandonFocus()
        }
    }

    private fun abandonFocus() {
        val handle = focusHandle ?: return
        if (Build.VERSION.SDK_INT >= 26 && handle is AudioFocusRequest) {
            audioManager?.abandonAudioFocusRequest(handle)
        } else {
            @Suppress("DEPRECATION")
            audioManager?.abandonAudioFocus(focusListener)
        }
        focusHandle = null
    }

    private fun updateSession() {
        val mediaSession = session ?: return
        mediaSession.setPlaybackState(
            PlaybackStateCompat.Builder()
                .setActions(
                    PlaybackStateCompat.ACTION_PLAY
                        or PlaybackStateCompat.ACTION_PAUSE
                        or PlaybackStateCompat.ACTION_PLAY_PAUSE
                        or PlaybackStateCompat.ACTION_SKIP_TO_NEXT
                        or PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS,
                )
                // 位置 + 更新时间 + 速度：系统据此自行推进锁屏卡片的进度条，暂停时速度为 0
                .setState(
                    if (playing) PlaybackStateCompat.STATE_PLAYING else PlaybackStateCompat.STATE_PAUSED,
                    positionMs,
                    if (playing) rate else 0f,
                    android.os.SystemClock.elapsedRealtime(),
                )
                .build(),
        )
        mediaSession.setMetadata(
            MediaMetadataCompat.Builder()
                .putString(MediaMetadataCompat.METADATA_KEY_TITLE, title ?: "")
                .putString(MediaMetadataCompat.METADATA_KEY_ARTIST, artist ?: "")
                .putLong(MediaMetadataCompat.METADATA_KEY_DURATION, durationMs)
                .build(),
        )
    }

    private fun buildNotification(): Notification {
        val mediaSession = session!!
        val toggleLabel = if (playing) "暂停" else "播放"
        val toggleIcon = if (playing) android.R.drawable.ic_media_pause else android.R.drawable.ic_media_play
        val openApp = PendingIntent.getActivity(
            this,
            0,
            packageManager.getLaunchIntentForPackage(packageName),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val builder = NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle(title ?: "music")
            .setContentText(artist ?: "")
            .setContentIntent(openApp)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setOnlyAlertOnce(true)
            .addAction(android.R.drawable.ic_media_previous, "上一首", servicePendingIntent(ACTION_PREV, 1))
            .addAction(toggleIcon, toggleLabel, servicePendingIntent(ACTION_TOGGLE, 2))
            .addAction(android.R.drawable.ic_media_next, "下一首", servicePendingIntent(ACTION_NEXT, 3))
            .setStyle(
                androidx.media.app.NotificationCompat.MediaStyle()
                    .setMediaSession(mediaSession.sessionToken)
                    .setShowActionsInCompactView(0, 1, 2),
            )
        return builder.build()
    }

    private fun servicePendingIntent(action: String, requestCode: Int): PendingIntent {
        val intent = Intent(this, PlaybackService::class.java).setAction(action)
        return PendingIntent.getService(
            this,
            requestCode,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    private fun startInForeground(notification: Notification) {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= 26) {
            val channel = NotificationChannel(CHANNEL_ID, "正在播放", NotificationManager.IMPORTANCE_LOW)
            manager.createNotificationChannel(channel)
        }
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    companion object {
        const val CHANNEL_ID = "playback"
        const val NOTIFICATION_ID = 42
        const val EXTRA_PLAYING = "playing"
        const val EXTRA_TITLE = "title"
        const val EXTRA_ARTIST = "artist"
        const val EXTRA_DURATION = "durationMs"
        const val EXTRA_POSITION = "positionMs"
        const val EXTRA_RATE = "rate"
        const val ACTION_TOGGLE = "app.tauri.playback.TOGGLE"
        const val ACTION_NEXT = "app.tauri.playback.NEXT"
        const val ACTION_PREV = "app.tauri.playback.PREV"
    }
}
