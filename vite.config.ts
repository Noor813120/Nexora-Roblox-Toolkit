import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed dev port and must not have its Rust output hidden by Vite's screen clearing.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // WebView2 on Windows 10/11 is a current Chromium, so modern output is safe and smaller.
    target: "es2022",
    sourcemap: false,
  },
});
