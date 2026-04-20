import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import Icons from "unplugin-icons/vite";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    tanstackRouter({
      autoCodeSplitting: true,
    }),
    tailwindcss(),
    react({
      babel: {
        plugins: ["babel-plugin-react-compiler"],
      },
    }),
    Icons({
      compiler: "jsx",
      jsx: "react",
    }),
  ],

  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  build: {
    // The default single-chunk output balloons when CodeMirror, table
    // virtualization, and the Base UI primitives all land together. A
    // handful of manual chunks buys us parallel fetches, better cache
    // hits between versions, and sub-500KB chunks. Grouping is by
    // "cost to bring in" rather than strict package boundaries.
    rollupOptions: {
      output: {
        manualChunks: {
          codemirror: [
            "@uiw/react-codemirror",
            "@codemirror/lang-sql",
            "@codemirror/lang-json",
            "@codemirror/view",
            "@uiw/codemirror-theme-vscode",
          ],
          table: ["@tanstack/react-table", "@tanstack/react-virtual"],
          baseUi: ["@base-ui/react"],
        },
      },
    },
    chunkSizeWarningLimit: 900,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
