import { readFileSync } from "node:fs";
import { fileURLToPath, URL } from "node:url";

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwind from "@tailwindcss/vite";

const appVersion: string = JSON.parse(readFileSync(new URL("../desktop/src-tauri/tauri.conf.json", import.meta.url), "utf8")).version;

export default defineConfig({
  define: { __APP_VERSION__: JSON.stringify(appVersion) },
  plugins: [react(), tailwind()],
  resolve: {
    // Тот же алиас, что в оригинале, чтобы перенесённые модули работали без правок импортов.
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  // Относительная база: собранный фронт раздаёт наш же сервер с того же origin.
  base: "./",
  server: {
    // В разработке фронт живёт отдельно, а API проксируется на сервер.
    proxy: {
      "/api": "http://127.0.0.1:8770",
      "/generated": "http://127.0.0.1:8770",
      "/uploads": "http://127.0.0.1:8770",
      "/v1": "http://127.0.0.1:8770",
    },
  },
});
