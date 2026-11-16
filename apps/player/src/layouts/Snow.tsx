import { useEffect, useRef } from "react";

/** 飘雪画布高度（上淡入、控制区上方淡出由外层遮罩负责）。 */
const SPRITE_SIZE = 64;

type Flake = {
    x: number;
    y: number;
    size: number;
    vy: number;
    sway: number;
    freq: number;
    phase: number;
    rot: number;
    spin: number;
    alpha: number;
    sprite: number;
};

/** 画一片六角雪花：6 条主臂，每条臂上有分叉，中心一个小六边形。 */
function drawCrystal(
    context: CanvasRenderingContext2D,
    opt: { branches: Array<[number, number]>; tip: boolean; hex: boolean },
): void {
    const center = SPRITE_SIZE / 2;
    const arm = SPRITE_SIZE * 0.42;
    context.translate(center, center);
    context.strokeStyle = "#fff";
    context.lineCap = "round";
    context.lineJoin = "round";
    context.shadowColor = "rgba(255,255,255,0.9)";
    context.shadowBlur = 3;
    for (let k = 0; k < 6; k++) {
        context.save();
        context.rotate((Math.PI / 3) * k);
        context.lineWidth = 3;
        context.beginPath();
        context.moveTo(0, 0);
        context.lineTo(0, -arm);
        context.stroke();
        context.lineWidth = 2.2;
        for (const [position, length] of opt.branches) {
            const y = -arm * position;
            const l = arm * length;
            context.beginPath();
            context.moveTo(-l * 0.8, y - l * 0.75);
            context.lineTo(0, y);
            context.lineTo(l * 0.8, y - l * 0.75);
            context.stroke();
        }
        if (opt.tip) {
            context.beginPath();
            context.arc(0, -arm, 2.2, 0, Math.PI * 2);
            context.fillStyle = "#fff";
            context.fill();
        }
        context.restore();
    }
    if (opt.hex) {
        context.lineWidth = 2;
        context.beginPath();
        for (let k = 0; k <= 6; k++) {
            const angle = (Math.PI / 3) * k;
            context.lineTo(Math.sin(angle) * arm * 0.22, -Math.cos(angle) * arm * 0.22);
        }
        context.stroke();
    }
}

/** 三种花型，让雪花不千篇一律。 */
function buildSprites(): HTMLCanvasElement[] {
    const options = [
        {
            branches: [
                [0.45, 0.32],
                [0.72, 0.24],
            ] as Array<[number, number]>,
            tip: false,
            hex: true,
        },
        {
            branches: [
                [0.35, 0.28],
                [0.58, 0.3],
                [0.8, 0.18],
            ] as Array<[number, number]>,
            tip: true,
            hex: false,
        },
        { branches: [[0.55, 0.42]] as Array<[number, number]>, tip: false, hex: true },
    ];
    return options.map(opt => {
        const canvas = document.createElement("canvas");
        canvas.width = SPRITE_SIZE;
        canvas.height = SPRITE_SIZE;
        const context = canvas.getContext("2d");
        if (context) drawCrystal(context, opt);
        return canvas;
    });
}

/** 飘雪：六角冰晶，近大远小、近快远慢，边落边转、左右轻摆；减少动态效果与页面不可见时停止。 */
export function Snow({ className }: { className?: string }) {
    const canvas = useRef<HTMLCanvasElement>(null);

    useEffect(() => {
        const element = canvas.current;
        if (!element) return;
        const reduce =
            typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
        if (reduce) return;

        const sprites = buildSprites();
        const context = element.getContext("2d");
        let flakes: Flake[] = [];
        let width = 0;
        let height = 0;
        let dpr = 1;
        const makeFlake = (anywhere: boolean): Flake => {
            const depth = Math.random();
            return {
                x: Math.random() * width,
                y: anywhere ? Math.random() * height : -20 - Math.random() * 40,
                size: 8 + depth * depth * 14,
                vy: 16 + depth * 30,
                sway: 8 + Math.random() * 16,
                freq: 0.3 + Math.random() * 0.6,
                phase: Math.random() * Math.PI * 2,
                rot: Math.random() * Math.PI * 2,
                spin: (Math.random() - 0.5) * 1.2,
                alpha: 0.35 + depth * 0.55,
                sprite: Math.floor(Math.random() * sprites.length),
            };
        };
        const resize = () => {
            dpr = Math.min(window.devicePixelRatio || 1, 2);
            const w = element.clientWidth;
            const h = element.clientHeight;
            if (!w || (w === width && h === height)) return;
            width = w;
            height = h;
            element.width = w * dpr;
            element.height = h * dpr;
            // 约 36 片，稀疏不抢眼
            flakes = Array.from({ length: Math.round((w * h) / 7600) }, () => makeFlake(true));
        };

        let visible = !document.hidden;
        let frame: number | null = null;
        let last = performance.now();
        let time = 0;
        const tick = (now: number) => {
            const dt = Math.min(0.05, (now - last) / 1000);
            last = now;
            if (visible && context) {
                time += dt;
                resize();
                context.setTransform(1, 0, 0, 1, 0, 0);
                context.clearRect(0, 0, element.width, element.height);
                for (const flake of flakes) {
                    flake.y += flake.vy * dt;
                    flake.rot += flake.spin * dt;
                    if (flake.y - flake.size > height) Object.assign(flake, makeFlake(false));
                    const x = flake.x + Math.sin(time * flake.freq + flake.phase) * flake.sway;
                    const scale = (flake.size / SPRITE_SIZE) * dpr;
                    const cos = Math.cos(flake.rot) * scale;
                    const sin = Math.sin(flake.rot) * scale;
                    context.setTransform(cos, sin, -sin, cos, x * dpr, flake.y * dpr);
                    context.globalAlpha = flake.alpha;
                    context.drawImage(sprites[flake.sprite], -SPRITE_SIZE / 2, -SPRITE_SIZE / 2);
                }
                context.globalAlpha = 1;
            }
            frame = requestAnimationFrame(tick);
        };
        const onVisibility = () => {
            visible = !document.hidden;
            // 页面不可见时停止重绘（rAF 继续但直接跳过，恢复时自动续上）
        };
        document.addEventListener("visibilitychange", onVisibility);
        frame = requestAnimationFrame(tick);
        return () => {
            if (frame !== null) cancelAnimationFrame(frame);
            document.removeEventListener("visibilitychange", onVisibility);
        };
    }, []);

    return <canvas ref={canvas} className={className} aria-hidden="true" />;
}
