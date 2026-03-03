import { resolve } from "node:path";
import type { Plugin } from "vite";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

function landingHtmlPlugin(): Plugin {
  return {
    name: "landing-html",
    configureServer(server) {
      server.middlewares.use((req, _res, next) => {
        if (req.url === "/" || req.url === "/index.html") {
          req.url = "/index.landing.html";
        }
        next();
      });
    },
  };
}

export default defineConfig({
  base: "/Orchestrix/",
  root: ".",
  plugins: [landingHtmlPlugin(), react(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname!, "./src"),
    },
  },
  build: {
    outDir: "dist-landing",
    emptyOutDir: true,
    minify: true,
    rollupOptions: {
      input: resolve(import.meta.dirname!, "index.landing.html"),
    },
  },
  server: {
    port: 5174,
    open: false,
  },
});
