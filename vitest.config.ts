import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

// Headless tests exercise pure modules and Vue setup without a DOM.
// Keep the Tauri-tailored development server out of the test run.
export default defineConfig({
  plugins: [vue()],
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
