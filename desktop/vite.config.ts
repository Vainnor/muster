import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Tauri serves dist/ from its own protocol in release builds; the dev server is only for
// `tauri dev`. No inline scripts or styles end up in the build, so the strict CSP holds.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true, host: "127.0.0.1" },
  build: {
    target: ["es2022", "safari16"],
    outDir: "dist",
    emptyOutDir: true,
    assetsInlineLimit: 0,
    sourcemap: false,
  },
  test: { environment: "node", include: ["src/**/*.test.ts"] },
});
