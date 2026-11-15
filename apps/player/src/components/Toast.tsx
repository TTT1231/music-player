import { useEffect, useRef, useState, type ReactNode } from "react";
import "./controls.css";

export interface ToastProps {
    message: string | null;
    variant?: "desktop" | "mobile";
    icon?: ReactNode;
    id?: string | number;
    onDismiss?: () => void;
}

export function Toast({ message, variant = "desktop", icon, id, onDismiss }: ToastProps) {
    const [visible, setVisible] = useState(Boolean(message));
    const dismiss = useRef(onDismiss);
    dismiss.current = onDismiss;
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
        <div role="status" className={`player-toast player-toast--${variant}`}>
            {icon}
            <span>{message}</span>
        </div>
    );
}
