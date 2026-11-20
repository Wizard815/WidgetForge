import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "node:path";

const host = process.env.TAURI_DEV_HOST;

// Two pages: the Hub (main menu) and the widget host (one window per widget instance).
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        hub: resolve(import.meta.dirname, "hub.html"),
        widget: resolve(import.meta.dirname, "widget.html"),
      },
    },
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
});
