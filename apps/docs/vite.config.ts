import { defineConfig, type Plugin } from "vite";
import { themeScript } from "@logicplanes/kaeso-ui";
import { neutral } from "@logicplanes/kaeso-tokens";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { writeSitePolicy } from "../../scripts/csp.mjs";
import { kaesoVersion } from "./scripts/content/docs.ts";
import { kaesoDocs } from "./scripts/content/plugin.ts";
import { UPDATE_SERVICE } from "./src/site/update-service.ts";

const root = fileURLToPath(new URL(".", import.meta.url));
const contentDir = process.env.KAESO_DOCS_CONTENT ?? "content";
const changelog = process.env.KAESO_DOCS_CHANGELOG ?? "../../CHANGELOG.md";
const editBase = "https://github.com/logicplanes/kaeso/edit/dev/apps/docs/";
const siteUrl = "https://docs.kaeso.app";
const port = Number(process.env.KAESO_DOCS_PORT ?? 4480);

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

function fontPreload(): Plugin {
  return {
    name: "kaeso-font-preload",
    apply: "build",
    transformIndexHtml: {
      order: "post",
      handler: (_html, ctx) => {
        const font = Object.keys(ctx.bundle ?? {}).find((f) => /(^|\/)inter-latin-opsz-normal-[\w-]+\.woff2$/.test(f));
        if (!font) throw new Error("kaeso-font-preload: the interface font isn't in the bundle; preload the one the stylesheet uses");
        return [{ tag: "link", attrs: { rel: "preload", href: `/${font}`, as: "font", type: "font/woff2", crossorigin: true }, injectTo: "head" }];
      },
    },
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
        this.info(writeSitePolicy(outDir, { images: [UPDATE_SERVICE] }));
      },
    },
  };
}

function uiEntryImports(): Plugin {
  const dist = fileURLToPath(new URL("./node_modules/@logicplanes/kaeso-ui/dist", import.meta.url));
  const names = (src: string) =>
    [...src.matchAll(/export \{([^}]*)\}/g)].flatMap((m) =>
      (m[1] ?? "")
        .split(",")
        .map(
          (part) =>
            part
              .trim()
              .split(/\s+as\s+/)
              .pop() ?? "",
        )
        .filter(Boolean),
    );
  const entries = new Map<string, string>();
  const iconExports = new Map<string, string>();
  const scan = (dir: string, prefix: string) => {
    if (!existsSync(`${dist}/${dir}`)) return;
    for (const f of readdirSync(`${dist}/${dir}`).filter((f) => f.endsWith(".js")))
      for (const n of names(readFileSync(`${dist}/${dir}/${f}`, "utf8"))) if (!entries.has(n)) entries.set(n, `${prefix}${f.replace(/\.js$/, "")}`);
  };
  const scanFile = (file: string, from: string) => {
    for (const n of names(readFileSync(`${dist}/${file}`, "utf8"))) if (!entries.has(n)) entries.set(n, from);
  };
  return {
    name: "kaeso-ui-entry-imports",
    apply: "build",
    enforce: "pre",
    async buildStart() {
      const { icons } = (await import(pathToFileURL(`${dist}/components/icon.js`).href)) as { icons: Record<string, unknown> };
      const hugeicons = (await import("@hugeicons/core-free-icons")) as Record<string, unknown>;
      const byIcon = new Map(Object.entries(hugeicons).map(([name, icon]) => [icon, name]));
      iconExports.clear();
      for (const [meaning, icon] of Object.entries(icons)) {
        const name = byIcon.get(icon);
        if (name) iconExports.set(meaning, name);
      }
      entries.clear();
      scanFile("theme.js", "@logicplanes/kaeso-ui/theme");
      scanFile("utils.js", "@logicplanes/kaeso-ui/utils");
      scan("components/kaeso", "@logicplanes/kaeso-ui/kaeso/");
      scan("components", "@logicplanes/kaeso-ui/");
      scan("blocks", "@logicplanes/kaeso-ui/blocks/");
    },
    transform(code, id) {
      const file = id.split("?")[0] ?? id;
      if (!/\/src\/.*\.tsx?$/.test(file) || !code.includes("@logicplanes/kaeso-ui")) return null;
      const icons = new Set<string>();
      let out = code.replace(/\bicons\.(\w+)\b/g, (whole, meaning: string) => {
        const name = iconExports.get(meaning);
        if (!name) return whole;
        icons.add(name);
        return `__icon_${name}`;
      });
      out = out.replace(/import\s*\{([^}]*)\}\s*from\s*["']@logicplanes\/kaeso-ui["'];?/g, (whole, list: string) => {
        const groups = new Map<string, string[]>();
        for (const raw of list
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean)) {
          if (raw.startsWith("type ")) continue;
          if (raw === "icons" && icons.size && !/\bicons\b(?!\.)/.test(out.replace(whole, ""))) continue;
          const name = raw.split(/\s+as\s+/)[0] ?? raw;
          const from = entries.get(name) ?? "@logicplanes/kaeso-ui";
          groups.set(from, [...(groups.get(from) ?? []), raw]);
        }
        const iconImport = icons.size ? `import { ${[...icons].map((i) => `${i} as __icon_${i}`).join(", ")} } from "@hugeicons/core-free-icons";` : "";
        icons.clear();
        return [...[...groups].map(([from, names]) => `import { ${names.join(", ")} } from "${from}";`), iconImport].filter(Boolean).join("\n");
      });
      return out === code ? null : { code: out, map: null };
    },
  };
}

export default defineConfig({
  cacheDir: process.env.KAESO_VITE_CACHE ?? "node_modules/.vite",
  plugins: [
    kaesoDocs({ root, contentDir, editBase, siteUrl, version: kaesoVersion(root), changelog }),
    uiEntryImports(),
    themeInit(),
    themeColor(),
    fontPreload(),
    react(),
    tailwindcss(),
    contentSecurityPolicy(),
  ],
  resolve: {
    alias: [
      { find: "~", replacement: fileURLToPath(new URL("./src", import.meta.url)) },
      { find: /^use-sync-external-store\/shim\/with-selector(\.js)?$/, replacement: "use-sync-external-store/with-selector.js" },
      { find: /^use-sync-external-store\/shim(\/index\.js)?$/, replacement: "react" },
    ],
  },
  server: { port, strictPort: true },
  preview: { port, strictPort: true },
  optimizeDeps: {
    include: [
      "react",
      "react-dom",
      "react-dom/client",
      "react/jsx-runtime",
      "react/jsx-dev-runtime",
      "@tanstack/react-router",
      "@logicplanes/kaeso-ui",
      "@logicplanes/kaeso-ui/i18n",
      "@logicplanes/kaeso-ui/format",
      "@logicplanes/kaeso-ui/toast",
      "@logicplanes/kaeso-ui/command-palette",
    ],
  },
  build: {
    target: "es2023",
    sourcemap: true,
    manifest: true,
    modulePreload: { polyfill: false },
    rolldownOptions: {
      output: {
        chunkFileNames: (chunk) => (chunk.isDynamicEntry || chunk.name === "framework" ? "assets/[name]-[hash:7].js" : "assets/[hash:7].js"),
        codeSplitting: {
          groups: [{ name: "framework", test: /node_modules[\\/](react|react-dom|scheduler|@tanstack)[\\/]/, tags: ["$initial"] }],
        },
      },
    },
  },
});
