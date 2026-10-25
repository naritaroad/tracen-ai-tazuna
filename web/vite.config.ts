import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// During development the Rust API runs locally on 8080 (API_PROXY_TARGET overrides it); the site and
// API share one origin through this proxy.
const api = process.env.API_PROXY_TARGET ?? "http://127.0.0.1:8080";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": { target: api, changeOrigin: false },
      "/avatar-assets": { target: api, changeOrigin: false },
      "/brand": { target: api, changeOrigin: false },
    },
  },
  // The 3D player (three.js + three-vrm, ~200 kB gzip) is its own chunk, loaded only with the character.
  build: { outDir: "dist", sourcemap: true, chunkSizeWarningLimit: 900 },
});
