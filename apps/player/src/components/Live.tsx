// 按帧变化的小部件：各自订阅实时进度，布局和播放队列不随进度重渲染。
import { usePlaybackPosition } from "../store";
import { formatTime } from "./formatTime";
import { LyricView, type LyricViewProps } from "./LyricView";
import { ProgressBar, type ProgressBarProps } from "./ProgressBar";

/** preview：拖动进度条时预览的位置，优先于实时进度。 */
type Preview = { preview?: number | null };

export function LiveLyrics({ preview, ...props }: Omit<LyricViewProps, "positionMs"> & Preview) {
    const position = usePlaybackPosition();
    return <LyricView {...props} positionMs={preview ?? position} />;
}

export function LiveProgressBar(props: Omit<ProgressBarProps, "positionMs">) {
    return <ProgressBar {...props} positionMs={usePlaybackPosition()} />;
}

export function LiveTime({ preview }: Preview) {
    const position = usePlaybackPosition();
    return <>{formatTime(preview ?? position)}</>;
}

/** 已播放比例（0–100），用于迷你播放条等只需宽度的地方。 */
export function LiveFill({
    preview,
    durationMs,
    className,
}: Preview & { durationMs: number | null; className: string }) {
    const live = usePlaybackPosition();
    const position = preview ?? live;
    return <span className={className} style={{ width: `${durationMs ? (position / durationMs) * 100 : 0}%` }} />;
}
