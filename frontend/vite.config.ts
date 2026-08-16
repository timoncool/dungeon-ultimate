import { fileURLToPath, URL } from "node:url";

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwind from "@tailwindcss/vite";

export default defineConfig({
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
    },
  },
});
