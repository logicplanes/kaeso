import { defineConfig, type Plugin } from "vite";
import { themeScript } from "@logicplanes/kaeso-ui/theme";
import { neutral } from "@logicplanes/kaeso-tokens";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "node:path";
import { writeSitePolicy } from "../../scripts/csp.mjs";

const port = Number(process.env.KAESO_UPDATE_PORT ?? 4490);

function themeInit(): Plugin {
  const file = "theme-init.js";
  return {
    name: "kaeso-theme-init",
    configureServer(server) {
      server.middlewares.use(`/${file}`, (_req, res) => {
        res.setHeader("content-type", "text/javascript");
        res.end(themeScript);
      });
    },
    generateBundle() {
      this.emitFile({ type: "asset", fileName: file, source: themeScript });
    },
    transformIndexHtml: () => [{ tag: "script", attrs: { src: `/${file}` }, injectTo: "head" }],
  };
}

function themeColor(): Plugin {
  const canvas = neutral["bg-panel"];
  return {
    name: "kaeso-theme-color",
    transformIndexHtml: () =>
      (["light", "dark"] as const).map((scheme) => ({
        tag: "meta",
        attrs: { name: "theme-color", content: canvas[scheme], media: `(prefers-color-scheme: ${scheme})` },
        injectTo: "head" as const,
      })),
  };
}

function contentSecurityPolicy(): Plugin {
  let outDir = "";
  return {
    name: "kaeso-content-security-policy",
    apply: "build",
    configResolved(config) {
      outDir = resolve(config.root, config.build.outDir);
    },
    closeBundle: {
      order: "post",
      sequential: true,
      handler() {
        this.info(writeSitePolicy(outDir));
      },
    },
  };
}

export default defineConfig({
  cacheDir: process.env.KAESO_VITE_CACHE ?? "node_modules/.vite",
  plugins: [themeInit(), themeColor(), react(), tailwindcss(), contentSecurityPolicy()],
  resolve: {
    alias: [
      { find: /^use-sync-external-store\/shim\/with-selector(\.js)?$/, replacement: "use-sync-external-store/with-selector.js" },
      { find: /^use-sync-external-store\/shim(\/index\.js)?$/, replacement: "react" },
    ],
  },
  server: { port, strictPort: true },
  preview: { port, strictPort: true },
  build: {
    target: "es2023",
    sourcemap: true,
    modulePreload: { polyfill: false },
  },
});
