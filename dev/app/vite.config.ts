/// <reference types="vitest/config" />
import { defineConfig } from 'vite';

// Tauri expects a fixed dev port and a relative build output.
export default defineConfig({
  clearScreen: false,
  // dev/ is allowed: the app imports dev/project.json and brand assets from dev/assets.
  server: { port: 5173, strictPort: true, fs: { allow: ['..'] } },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    outDir: 'build',
    emptyOutDir: true,
    // WebView2 (Windows) and WKWebView (macOS: Safari 16.4 or later).
    target: ['chrome110', 'safari16.4'],
    sourcemap: false,
    chunkSizeWarningLimit: 1500,
  },
  worker: { format: 'es' },
  test: {
    environment: 'jsdom',
    include: ['tests/**/*.test.ts'],
  },
});
