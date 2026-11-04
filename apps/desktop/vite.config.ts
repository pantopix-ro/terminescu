import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri loads the dev server on a fixed port and serves dist/ in builds.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  // One local bundle, loaded from disk: xterm.js and its addons make it large, and that is fine.
  build: { target: "safari16", sourcemap: true, chunkSizeWarningLimit: 1500 },
});
