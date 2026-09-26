import { StrictMode, type ReactNode } from "react";
import { createRoot, hydrateRoot } from "react-dom/client";
import "./index.css";

/**
 * Production builds prerender every page (scripts/prerender.mjs), so there is
 * markup to hydrate; the dev server serves an empty root and renders fresh.
 */
export function mount(page: ReactNode) {
  const root = document.getElementById("root")!;
  const tree = <StrictMode>{page}</StrictMode>;
  if (root.hasChildNodes()) hydrateRoot(root, tree);
  else createRoot(root).render(tree);
}
