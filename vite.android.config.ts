import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "node:path";
export default defineConfig({
  base: "./",
  plugins: [vue()],
  build: { outDir: "dist-android", rollupOptions: { input: resolve(import.meta.dirname, "native-mobile.html") } },
});
