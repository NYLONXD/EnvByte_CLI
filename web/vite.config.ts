import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: {
    // One HTML file per page, so each gets its own title, description and
    // structured data. scripts/prerender.mjs fills in their markup.
    rollupOptions: {
      input: {
        main: "index.html",
        about: "about.html",
      },
    },
  },
});
