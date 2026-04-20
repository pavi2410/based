import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import Icons from "unplugin-icons/vite";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

/** Rolldown (Vite 8) requires `manualChunks` as a function, not a record. */
function manualChunks(id: string): string | undefined {
  if (
    id.includes("@uiw/react-codemirror") ||
    id.includes("@uiw/codemirror-theme-vscode") ||
    id.includes("@codemirror/") ||
    id.includes("@codemirror+")
  ) {
    return "codemirror";
  }
  if (id.includes("@tanstack/react-table") || id.includes("@tanstack/react-virtual")) {
    return "table";
  }
  if (id.includes("@base-ui/react")) {
    return "baseUi";
  }
  return undefined;
}

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
        manualChunks,
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
