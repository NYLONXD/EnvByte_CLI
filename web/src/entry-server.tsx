// Build-time only: scripts/prerender.mjs renders each page into its HTML file
// so crawlers and AI assistants that don't run JavaScript still read it.
import { StrictMode, type ReactNode } from "react";
import { renderToString } from "react-dom/server";
import { AboutPage } from "./About";
import { App } from "./App";

/** Keyed by the HTML file each page is built into, relative to dist/. */
const PAGES: Record<string, ReactNode> = {
  "index.html": <App />,
  "about.html": <AboutPage />,
};

export const pageFiles = Object.keys(PAGES);

export function render(file: string): string {
  return renderToString(<StrictMode>{PAGES[file]}</StrictMode>);
}
