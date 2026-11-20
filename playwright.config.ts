import { defineConfig } from "@playwright/test";

// Minimal e2e config: the React dev server is the only webServer we own. The
// backend is started separately by the smoke run (see the evidence report).
export default defineConfig({
  testDir: "tests/e2e",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 30_000,
  reporter: [["list"]],
  use: {
    baseURL: "http://127.0.0.1:5173",
    browserName: "chromium",
    headless: true,
    viewport: { width: 1280, height: 720 },
    trace: "retain-on-failure",
  },
  webServer: {
    // Vite defaults to `localhost` (::1 on this host); force IPv4 so the
    // 127.0.0.1 health check below matches the actual listener.
    command: "npm run dev -- --host 127.0.0.1",
    cwd: "apps/desktop",
    url: "http://127.0.0.1:5173",
    reuseExistingServer: true,
    timeout: 120_000,
  },
});
