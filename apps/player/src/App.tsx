import { useEffect } from "react";
import { AndroidLayout } from "./layouts/AndroidLayout";
import { DesktopLayout } from "./layouts/DesktopLayout";
import { playerStore, usePlayerState } from "./store";

export default function App() {
    const { platform } = usePlayerState();

    useEffect(() => {
        void playerStore.start();
        return () => playerStore.stop();
    }, []);

    if (platform === "android") return <AndroidLayout />;
    if (platform === "desktop") return <DesktopLayout />;
    return <main />;
}
