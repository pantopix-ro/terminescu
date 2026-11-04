import { defineConfig, devices } from "@playwright/test";

// Runs the Vite build on the mock Host in WebKit, the engine behind the Tauri webview.
export default defineConfig({
  testDir: "e2e",
  timeout: 60_000,
  use: { ...devices["Desktop Safari"], baseURL: "http://localhost:4173" },
  webServer: {
    command: "pnpm build && pnpm preview --port 4173 --strictPort",
    url: "http://localhost:4173",
    reuseExistingServer: false,
  },
});
