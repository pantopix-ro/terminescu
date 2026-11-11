import { defineConfig, devices } from "@playwright/test";

// Runs the Vite build on the mock Host in WebKit, the engine behind the Tauri webview.
export default defineConfig({
  testDir: "e2e",
  timeout: 60_000,
  // CI shares one Mac runner with other jobs, and a drag or a click can then land a frame late.
  // One retry there; the report still lists such a test as flaky.
  retries: process.env.CI ? 1 : 0,
  use: { ...devices["Desktop Safari"], baseURL: "http://localhost:4173" },
  webServer: {
    command: "pnpm build && pnpm preview --port 4173 --strictPort",
    url: "http://localhost:4173",
    reuseExistingServer: false,
  },
});
