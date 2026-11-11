import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app";
import { installNativeMenuGuard } from "./shared/context-menu";
import "./styles/app.css";

// The index is local and changes only through scans, so cached pages stay valid until a scan ends.
const queryClient = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, retry: false, refetchOnWindowFocus: false } } });

// The app shows its own menus (or none): no "Reload" or "Save Image" from the webview.
installNativeMenuGuard();

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </StrictMode>,
  );
}
