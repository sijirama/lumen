//INFO: Vite configuration for Lumen
//NOTE: Configures Vite for Tauri application with React

import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],

  //INFO: Prevent Vite from obscuring Rust errors
  clearScreen: false,

  //INFO: Tauri development configuration
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      //INFO: Watch parent directory for changes
      ignored: ["**/src-tauri/**"],
    },
  },

  //INFO: Environment variable handling
  envPrefix: ["VITE_", "TAURI_"],

  build: {
    //INFO: Tauri uses Chromium on Windows and WebKit on macOS/Linux
    target: process.env.TAURI_PLATFORM === "windows" ? "chrome105" : "safari13",
    //INFO: Don't minify debug builds
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    //INFO: Generate sourcemaps for debug builds
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
