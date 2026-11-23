import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import "./controls.css";

export interface ToastProps {
    message: string | null;
    variant?: "desktop" | "mobile";
    icon?: ReactNode;
    id?: string | number;
    onDismiss?: () => void;
}

// 只有支持 Popover API 的浏览器才能把提示放进顶层；组件测试环境没有这个接口，按普通元素渲染
const supportsPopover = typeof HTMLElement !== "undefined" && typeof HTMLElement.prototype.showPopover === "function";

export function Toast({ message, variant = "desktop", icon, id, onDismiss }: ToastProps) {
    const node = useRef<HTMLDivElement>(null);
    const usePopover = variant === "mobile" && supportsPopover;
    const [visible, setVisible] = useState(Boolean(message));
    const dismiss = useRef(onDismiss);
    dismiss.current = onDismiss;
    // 手机端的对话框是原生 <dialog>，画在浏览器顶层，普通的 z-index 盖不住它的遮罩；
    // 提示作为 popover 也放进顶层
    useLayoutEffect(() => {
        const element = node.current;
        if (usePopover && element && !element.matches(":popover-open")) element.showPopover();
    }, [message, visible, id, usePopover]);
    useEffect(() => {
        if (!message) {
            setVisible(false);
            return;
        }
        setVisible(true);
        const timer = setTimeout(
            () => {
                setVisible(false);
                dismiss.current?.();
            },
            variant === "mobile" ? 1_800 : 2_600,
        );
        return () => clearTimeout(timer);
    }, [message, variant, id]);
    if (!message || !visible) return null;
    return (
        <div
            ref={node}
            popover={usePopover ? "manual" : undefined}
            role="status"
            className={`player-toast player-toast--${variant}`}
        >
            {icon}
            <span>{message}</span>
        </div>
    );
}
