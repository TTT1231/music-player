// 生成测试素材 fixtures/music/：几秒 440 Hz 正弦波音频和配套歌词，结果进 git。
// 需要 PATH 中有带 libmp3lame、libvorbis 的 ffmpeg；用法：node scripts/gen-fixtures.mjs
// 素材必须是生成的：assets/ 不进 git，CI 拿不到，也不能把有版权的歌曲放进仓库。
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const out = fileURLToPath(new URL("../fixtures/music/", import.meta.url));
const tmp = mkdtempSync(join(tmpdir(), "music-fixtures-"));

const TONE = "sine=frequency=440:sample_rate=44100:duration=3";
// bitexact 让重复生成的文件逐字节相同，避免无意义的二进制改动
const EXACT = ["-fflags", "+bitexact", "-flags:a", "+bitexact", "-map_metadata", "-1"];

const ffmpeg = (...args) => execFileSync("ffmpeg", ["-hide_banner", "-loglevel", "error", "-y", ...args]);

/** FLAC 图片块（Vorbis 注释 METADATA_BLOCK_PICTURE 的内容），ogg 内嵌封面用 */
function pictureBlock(png, mime = "image/png") {
    const u32 = n => {
        const b = Buffer.alloc(4);
        b.writeUInt32BE(n);
        return b;
    };
    const m = Buffer.from(mime);
    const [w, h] = [png.readUInt32BE(16), png.readUInt32BE(20)];
    return Buffer.concat([u32(3), u32(m.length), m, u32(0), u32(w), u32(h), u32(24), u32(0), u32(png.length), png]);
}

/** 用 TextDecoder 反推 GBK 编码表，免去 iconv 依赖 */
function encodeGbk(text) {
    const table = new Map();
    const dec = new TextDecoder("gbk");
    for (let hi = 0x81; hi <= 0xfe; hi++)
        for (let lo = 0x40; lo <= 0xfe; lo++) {
            const ch = dec.decode(Uint8Array.of(hi, lo));
            if (ch.length === 1 && ch !== "�" && !table.has(ch)) table.set(ch, [hi, lo]);
        }
    const bytes = [];
    for (const ch of text) {
        if (ch.codePointAt(0) < 0x80) bytes.push(ch.codePointAt(0));
        else if (table.has(ch)) bytes.push(...table.get(ch));
        else throw new Error(`GBK 无法编码：${ch}`);
    }
    return Buffer.from(bytes);
}

const lrc = (title, lines) =>
    [`[ti:${title}]`, "[ar:Test Artist]", ...lines.map(([t, text]) => `[${t}]${text}`), ""].join("\r\n");

try {
    rmSync(out, { recursive: true, force: true });
    mkdirSync(join(out, "subfolder"), { recursive: true });
    mkdirSync(join(out, "img"), { recursive: true });

    const cover = join(tmp, "cover.png");
    ffmpeg("-f", "lavfi", "-i", "color=c=0x2440b8:s=16x16", "-frames:v", "1", ...EXACT, cover);
    const coverB64 = pictureBlock(readFileSync(cover)).toString("base64");

    const mp3 = ["-c:a", "libmp3lame", "-b:a", "64k", "-ac", "1"];
    const ogg = ["-c:a", "libvorbis", "-q:a", "2", "-ac", "1"];

    // 有封面、UTF-8 歌词；文件名含多位歌手和版本
    const first = "Test Artist、Guest - Track One (Live)";
    ffmpeg(
        "-f",
        "lavfi",
        "-i",
        TONE,
        "-i",
        cover,
        "-map",
        "0:a",
        "-map",
        "1:v",
        ...mp3,
        "-c:v",
        "png",
        "-disposition:v",
        "attached_pic",
        "-id3v2_version",
        "3",
        ...EXACT,
        join(out, `${first}.mp3`),
    );
    writeFileSync(
        join(out, `${first}.lrc`),
        lrc("Track One", [
            ["00:00.50", "第一句歌词"],
            ["00:01.50", "第二句歌词"],
            ["00:02.50", "第三句歌词"],
        ]),
    );

    // 有封面、GBK 歌词
    const second = "Test Artist - Track Two";
    ffmpeg(
        "-f",
        "lavfi",
        "-i",
        TONE,
        ...ogg,
        ...EXACT,
        "-metadata:s:a:0",
        `METADATA_BLOCK_PICTURE=${coverB64}`,
        join(out, `${second}.ogg`),
    );
    writeFileSync(
        join(out, `${second}.lrc`),
        encodeGbk(
            lrc("Track Two", [
                ["00:00.50", "旧编码的歌词"],
                ["00:02.00", "第二句"],
            ]),
        ),
    );

    // 无封面、无歌词、文件名没有“ - ”
    ffmpeg("-f", "lavfi", "-i", TONE, ...mp3, ...EXACT, join(out, "No Artist Instrumental.mp3"));

    // 曲库 img/ 里的同名图片：模拟用户为无内嵌封面的歌曲准备的展示回退封面
    const sidecar = join(tmp, "sidecar.jpg");
    ffmpeg("-f", "lavfi", "-i", "color=c=0xb85c24:s=16x16", "-frames:v", "1", "-q:v", "2", ...EXACT, sidecar);
    copyFileSync(sidecar, join(out, "img", "No Artist Instrumental.jpg"));

    // 子文件夹：无封面，带 BOM 的 UTF-8 歌词
    const third = "Test Artist - Track Three";
    ffmpeg("-f", "lavfi", "-i", TONE, ...ogg, ...EXACT, join(out, "subfolder", `${third}.ogg`));
    writeFileSync(join(out, "subfolder", `${third}.lrc`), "﻿" + lrc("Track Three", [["00:01.00", "子文件夹里的歌词"]]));
} finally {
    rmSync(tmp, { recursive: true, force: true });
}
console.log(`已生成 ${out}`);
