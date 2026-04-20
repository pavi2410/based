import React from "react";
import ReactDOM from "react-dom/client";
import { Toaster } from "@/components/ui/sonner";
import { ThemeProvider } from "@/components/theme-provider";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { RouterProvider, createRouter } from "@tanstack/react-router";
import { WindowShell } from "@/components/window/window-shell";
// Import the generated route tree
import { routeTree } from "./routeTree.gen";
import "./index.css";

// Create a new router instance (main window only)
const router = createRouter({ routeTree });

// Register the router instance for type safety
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

const queryClient = new QueryClient();

/**
 * Child windows are opened with a `?window=<encoded WindowKind>` query
 * string (see `useWindow`/`WindowManager`). For those we skip the
 * router entirely and mount a lightweight `WindowShell`; the main
 * window renders the full app router.
 */
const isChildWindow = new URLSearchParams(window.location.search).has("window");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <ThemeProvider defaultTheme="dark" storageKey="vite-ui-theme">
        {isChildWindow ? <WindowShell /> : <RouterProvider router={router} />}
        <Toaster />
      </ThemeProvider>
    </QueryClientProvider>
  </React.StrictMode>,
);
