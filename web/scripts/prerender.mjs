// Renders every page into its built HTML file, so search engines and AI
// crawlers that don't run JavaScript read the whole page. Runs after both
// `vite build` passes (see the build script in package.json).
import { readFile, rm, writeFile } from "node:fs/promises";

const dist = new URL("../dist/", import.meta.url);
const ssr = new URL("../dist-ssr/", import.meta.url);
const EMPTY_ROOT = '<div id="root"></div>';

const { pageFiles, render } = await import(new URL("entry-server.js", ssr).href);

for (const file of pageFiles) {
  const path = new URL(file, dist);
  const html = await readFile(path, "utf8");
  if (!html.includes(EMPTY_ROOT)) throw new Error(`${file} has no empty ${EMPTY_ROOT} to fill`);
  // A function replacement, so a "$" in the markup is never read as a pattern.
  await writeFile(path, html.replace(EMPTY_ROOT, () => `<div id="root">${render(file)}</div>`));
  console.log(`prerendered ${file}`);
}

await rm(ssr, { recursive: true, force: true });
