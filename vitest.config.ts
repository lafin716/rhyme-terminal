import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

// Headless tests exercise pure modules and Vue setup without a DOM.
// Keep the Tauri-tailored development server out of the test run.
export default defineConfig({
  plugins: [vue()],
  test: {
    environment: "node",
    pool: "threads",
    // Bound module-transform concurrency on hosted runners and developer machines.
    maxWorkers: 2,
    // Cold Vue/TS module imports are slower on Windows and shared CI hosts.
    testTimeout: 30_000,
    include: ["src/**/*.test.ts"],
  },
});
