import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
export default defineConfig({
  plugins: [svelte()],
  build: {
    rollupOptions: {
      input: {
        index: "index.html",
        caption: "caption.html",
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: [
        "**/src-tauri/**",
        "**/.tools/**",
        "**/test-results/**",
        "**/release/**",
        "**/handoff/**",
        "**/.research/**",
      ],
    },
  },
  clearScreen: false,
});
