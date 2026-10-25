import { defineConfig, type Plugin } from "vite";
import { themeScript } from "@logicplanes/kaeso-ui";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, realpathSync } from "node:fs";
import { isAbsolute, join, relative, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const gateway = process.env.KAESO_GATEWAY_URL ?? `http://127.0.0.1:${process.env.KAESO_GATEWAY_PORT ?? 18080}`;
const cacheDir = process.env.KAESO_VITE_CACHE ?? "node_modules/.vite";
const uiDist = fileURLToPath(new URL("./node_modules/@logicplanes/kaeso-ui/dist", import.meta.url));

function libraryBuild(): Plugin {
  const hash = createHash("sha256");
  for (const file of readdirSync(uiDist, { recursive: true, encoding: "utf8" })
    .filter((f) => f.endsWith(".js"))
    .sort())
    hash.update(file).update(readFileSync(join(uiDist, file)));
  return { name: `kaeso-ui-build-${hash.digest("hex").slice(0, 16)}` };
}

// Vite serves a linked workspace package's unlisted entries from dist, beside the pre-bundled copy: two copies of its contexts.
function libraryEntries(): string[] {
  const name = "@logicplanes/kaeso-ui";
  const exports = (JSON.parse(readFileSync(join(uiDist, "../package.json"), "utf8")) as { exports: Record<string, string | { import?: string }> }).exports;
  return Object.entries(exports).flatMap(([key, target]) => {
    const file = typeof target === "string" ? target : target.import;
    if (!file?.endsWith(".js")) return [];
    const [before = "", after = ""] = file.replace(/^\.\/dist\//, "").split("*");
    if (!key.includes("*")) return [key === "." ? name : `${name}${key.slice(1)}`];
    const dir = join(uiDist, before);
    if (!existsSync(dir)) return [];
    return readdirSync(dir)
      .filter((f) => f.endsWith(after) && !f.endsWith(".map"))
      .map((f) => `${name}${key.slice(1).replaceAll("*", f.slice(0, -after.length))}`);
  });
}

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
    transformIndexHtml: () => [{ tag: "script", attrs: { src: `/${file}` }, injectTo: "head-prepend" }],
  };
}

function fontPreload(): Plugin {
  const name = "inter-latin-opsz-normal";
  let root = process.cwd();
  return {
    name: "kaeso-font-preload",
    configResolved(config) {
      root = config.root;
    },
    transformIndexHtml: {
      order: "post",
      handler: (_html, ctx) => {
        let href: string;
        if (ctx.bundle) {
          const font = Object.keys(ctx.bundle).find((f) => new RegExp(`(^|/)${name}-[\\w-]+\\.woff2$`).test(f));
          if (!font) throw new Error("kaeso-font-preload: the interface font isn't in the bundle; preload the one the stylesheet uses");
          href = `/${font}`;
        } else {
          const file = realpathSync(fileURLToPath(new URL(`./node_modules/@logicplanes/kaeso-ui/dist/fonts/${name}.woff2`, import.meta.url)));
          const path = relative(root, file);
          href = path.startsWith("..") || isAbsolute(path) ? `/@fs${file}` : `/${path.split(sep).join("/")}`;
        }
        return [
          { tag: "link", attrs: { rel: ctx.bundle ? "preload" : "prefetch", href, as: "font", type: "font/woff2", crossorigin: true }, injectTo: "head" },
        ];
      },
    },
  };
}

function uiEntryImports(): Plugin {
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
    if (!existsSync(`${uiDist}/${dir}`)) return;
    for (const f of readdirSync(`${uiDist}/${dir}`).filter((f) => f.endsWith(".js")))
      for (const n of names(readFileSync(`${uiDist}/${dir}/${f}`, "utf8"))) if (!entries.has(n)) entries.set(n, `${prefix}${f.replace(/\.js$/, "")}`);
  };
  const scanFile = (file: string, from: string) => {
    for (const n of names(readFileSync(`${uiDist}/${file}`, "utf8"))) if (!entries.has(n)) entries.set(n, from);
  };
  return {
    name: "kaeso-ui-entry-imports",
    apply: "build",
    enforce: "pre",
    async buildStart() {
      const { icons } = (await import(pathToFileURL(`${uiDist}/components/icon.js`).href)) as { icons: Record<string, unknown> };
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

type BundleChunk = { type: string; imports?: string[]; facadeModuleId?: string | null };
const preloadGraph = { chunks: {} as Record<string, BundleChunk>, frame: new Set<string>(), shared: new Set<string>() };
const LATE = /\/src\/routes\/_app\/(settings\/instance\/|orgs\/\$org\/projects\/\$project\/playground\.tsx)/;
function closureOf(file: string, seen = new Set<string>()): Set<string> {
  const chunk = preloadGraph.chunks[file];
  if (seen.has(file) || !chunk) return seen;
  seen.add(file);
  for (const dep of chunk.imports ?? []) closureOf(dep, seen);
  return seen;
}
function routePreloads(): Plugin {
  return {
    name: "kaeso-route-preloads",
    apply: "build",
    generateBundle: {
      order: "pre",
      handler(_options, bundle) {
        preloadGraph.chunks = bundle as unknown as Record<string, BundleChunk>;
        const routes = Object.entries(preloadGraph.chunks).filter(
          ([, c]) => c.type === "chunk" && /\/src\/routes\/.*\?tsr-split=/.test(c.facadeModuleId ?? ""),
        );
        const frame = routes.find(([, c]) => /\/src\/routes\/_app\.tsx\?tsr-split=component/.test(c.facadeModuleId ?? ""));
        preloadGraph.frame = frame ? closureOf(frame[0]) : new Set();
        preloadGraph.shared = new Set(routes.filter(([, c]) => !LATE.test(c.facadeModuleId ?? "")).flatMap(([file]) => [...closureOf(file)]));
      },
    },
  };
}
function resolvePreloads(file: string, deps: string[], { hostId }: { hostId: string }): string[] {
  const id = preloadGraph.chunks[file]?.facadeModuleId ?? "";
  const loaded = closureOf(hostId);
  return deps.filter(
    (dep) => !loaded.has(dep) && !(/\/src\/routes\/_app\//.test(id) && preloadGraph.frame.has(dep)) && (!LATE.test(id) || preloadGraph.shared.has(dep)),
  );
}

export default defineConfig({
  cacheDir,
  plugins: [
    libraryBuild(),
    uiEntryImports(),
    routePreloads(),
    themeInit(),
    fontPreload(),
    tanstackRouter({
      target: "react",
      autoCodeSplitting: true,
      codeSplittingOptions: { defaultBehavior: [["loader"], ["component"], ["errorComponent"], ["notFoundComponent"], ["pendingComponent"]] },
      routesDirectory: "src/routes",
      generatedRouteTree: "src/routeTree.gen.ts",
    }),
    react(),
    tailwindcss(),
  ],
  resolve: {
    alias: [
      { find: "~", replacement: fileURLToPath(new URL("./src", import.meta.url)) },
      { find: /^use-sync-external-store\/shim\/with-selector(\.js)?$/, replacement: "use-sync-external-store/with-selector.js" },
      { find: /^use-sync-external-store\/shim(\/index\.js)?$/, replacement: "react" },
    ],
  },
  server: {
    port: 3000,
    strictPort: true,
    proxy: {
      "/platform": { target: gateway, changeOrigin: false },
      "/v1": { target: gateway, changeOrigin: false },
      "/mcp": { target: gateway, changeOrigin: false },
      "/oauth": { target: gateway, changeOrigin: false },
      "/.well-known/oauth-": { target: gateway, changeOrigin: false },
    },
    warmup: {
      clientFiles: ["./src/main.tsx", "./src/routes/**/*.tsx", "./src/shell/*.tsx", "./src/features/**/*.tsx", "./src/kaeso/*.tsx", "!./src/**/*.test.tsx"],
    },
  },
  optimizeDeps: {
    include: [
      "react",
      "react-dom",
      "react-dom/client",
      "react/jsx-runtime",
      "react/jsx-dev-runtime",
      "@tanstack/react-query",
      "@tanstack/react-router",
      "openapi-fetch",
      ...libraryEntries(),
      "marked",
    ],
  },
  preview: {
    port: 3000,
    strictPort: true,
    proxy: { "/platform": gateway, "/v1": gateway, "/mcp": gateway, "/oauth": gateway, "/.well-known/oauth-": gateway },
  },
  build: {
    target: "es2023",
    sourcemap: true,
    manifest: true,
    chunkSizeWarningLimit: 600,
    modulePreload: { polyfill: false, resolveDependencies: resolvePreloads },
    rolldownOptions: {
      output: {
        chunkFileNames: (chunk) =>
          chunk.isDynamicEntry || chunk.name === "framework" || chunk.name === "ui" ? "assets/[name]-[hash:7].js" : "assets/[hash:7].js",
        codeSplitting: {
          groups: [
            { name: "framework", test: /node_modules[\\/](react|react-dom|scheduler|@tanstack)[\\/]|[\\/]src[\\/](api|auth|format)[\\/]/, tags: ["$initial"] },
            {
              name: "ui",
              test: /[\\/]packages[\\/]ui[\\/]dist[\\/]chunks[\\/](cn|button|logo|text|section|input-group|field)-[\w-]+\.js$|[\\/]src[\\/]shell[\\/](title\.ts|public-page\.tsx)$|[\\/]core-free-icons[\\/]dist[\\/]esm[\\/](Search01Icon|ArrowUpRight01Icon|Cancel01Icon)\.js$/,
            },
            { name: "gates", test: /[\\/]src[\\/](auth[\\/](route-gates|recent-sign-in-store)|shell[\\/](last|place)|api[\\/]queries[\\/]projects)\.ts$/ },
          ],
        },
      },
    },
  },
});
