import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed port and serves the built files from ui/dist.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  // The 3D viewer chunk (three.js) is ~600 kB but only loads when a model opens in 3D.
  build: { target: "es2022", outDir: "dist", chunkSizeWarningLimit: 700 },
});
