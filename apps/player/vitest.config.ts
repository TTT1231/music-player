import { defineConfig, mergeConfig } from "vitest/config";
import base from "../../vitest.unit.config.ts";
import viteConfig from "./vite.config.ts";

export default mergeConfig(
    mergeConfig(base, viteConfig),
    defineConfig({
        test: { environment: "jsdom" },
    }),
);
