import { defineConfig } from "vitest/config";
import path from "node:path";

// We only run "pure" JS tests for now — no React component rendering,
// no Tauri APIs. Tests that need the browser get happy-dom as a
// lightweight stand-in for jsdom.
export default defineConfig({
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  test: {
    environment: "happy-dom",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    setupFiles: ["./src/test-setup.ts"],
    globals: false,
  },
});
