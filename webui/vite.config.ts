import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The build output (dist/) is embedded into the native addon by rust-embed
// (see src-rust/web.rs). In dev, `vite` serves on :43111 and proxies /api to the
// Rust web server (`pi-switch webui start`, default :43110).
export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "/",
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    port: 43111,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:43110",
        changeOrigin: true,
      },
    },
  },
});
