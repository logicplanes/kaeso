import { readdirSync, existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import * as tokens from "@logicplanes/kaeso-tokens";

const src = resolve(import.meta.dirname, "src");
const dirs = (dir: string) =>
  existsSync(dir)
    ? readdirSync(dir, { withFileTypes: true })
        .filter((d) => d.isDirectory() && !d.name.startsWith("_"))
        .map((d) => d.name)
    : [];

const entries: Record<string, string> = {
  index: resolve(src, "index.ts"),
  format: resolve(src, "format/index.ts"),
  theme: resolve(src, "theme/index.ts"),
  utils: resolve(src, "utils/index.ts"),
  i18n: resolve(src, "i18n/index.tsx"),
  "i18n-provider": resolve(src, "i18n/provider.tsx"),
  "i18n-locales": resolve(src, "i18n/locale-tags.ts"),
};
for (const name of dirs(resolve(src, "components"))) {
  if (existsSync(resolve(src, "components", name, "index.ts"))) entries[`components/${name}`] = resolve(src, "components", name, "index.ts");
}
for (const file of ["identity", "moments", "usage", "conversation", "rights"]) {
  entries[`components/kaeso/${file}`] = resolve(src, "components/kaeso", `${file}.tsx`);
}
for (const file of ["timeseries", "bar", "sparkline", "donut", "heatmap", "gauge", "sankey"]) {
  entries[`components/charts/${file}`] = resolve(src, "components/charts", `${file}.tsx`);
}
for (const name of dirs(resolve(src, "blocks"))) {
  if (existsSync(resolve(src, "blocks", name, "index.ts"))) entries[`blocks/${name}`] = resolve(src, "blocks", name, "index.ts");
}

const pkg = JSON.parse(readFileSync(resolve(import.meta.dirname, "package.json"), "utf8")) as {
  dependencies?: Record<string, string>;
  peerDependencies?: Record<string, string>;
};
const external = [...Object.keys(pkg.dependencies ?? {}), ...Object.keys(pkg.peerDependencies ?? {})].map(
  (name) => new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&")}(\\/|$)`),
);

function stripVariantDescriptions() {
  return {
    name: "kaeso-strip-variant-descriptions",
    enforce: "pre" as const,
    transform(code: string, id: string) {
      if (!id.includes("/src/components/") && !id.includes("/src/blocks/")) return null;
      if (!code.includes("_VARIANTS")) return null;
      const out = code.replace(
        /(export const KAESO_\w+_VARIANTS = \{)([\s\S]*?)(\n\} as const;)/g,
        (_m, head: string, body: string, tail: string) => head + body.replace(/,\s*description:\s*(?:"(?:[^"\\]|\\.)*"|`(?:[^`\\]|\\.)*`)/g, "") + tail,
      );
      return out === code ? null : { code: out, map: null };
    },
  };
}

function directIcons() {
  const vocabulary = readFileSync(resolve(src, "icons/vocabulary.ts"), "utf8");
  const body = vocabulary.slice(vocabulary.indexOf("export const icons"));
  const meaning = new Map([...body.matchAll(/^\s+(\w+): (\w+Icon),/gm)].map((m) => [m[1]!, m[2]!]));
  return {
    name: "kaeso-direct-icons",
    enforce: "pre" as const,
    transform(code: string, id: string) {
      if (!id.includes("/src/components/") && !id.includes("/src/blocks/")) return null;
      if (!/import \{ icons \} from "\.\.\/\.\.\/icons\/vocabulary\.js";/.test(code)) return null;
      const used = new Set<string>();
      let out = code.replace(/\bicons\.(\w+)\b/g, (whole, name: string) => {
        const icon = meaning.get(name);
        if (!icon) throw new Error(`${id}: icons.${name} is not in the vocabulary`);
        used.add(icon);
        return `__icon_${icon}`;
      });
      out = out.replace(
        /import \{ icons \} from "\.\.\/\.\.\/icons\/vocabulary\.js";/,
        `import { ${[...used].map((i) => `${i} as __icon_${i}`).join(", ")} } from "@hugeicons/core-free-icons";`,
      );
      return { code: out, map: null };
    },
  };
}

const PURE_REACT = ["forwardRef", "memo", "createContext", "lazy"];
const PURE_FACTORIES = ["lazyModule", "makeRoot", "makeSvg", "rangePresets", "themeScriptFor", "BaseToast\\.createToastManager"];
function treeShakeable() {
  return {
    name: "kaeso-tree-shakeable",
    enforce: "pre" as const,
    transform(code: string, id: string) {
      if (!/\/src\/.*\.tsx?$/.test(id) || id.includes(".test.")) return null;
      let out = code
        .replace(
          /^([A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)*)\.displayName = ("(?:[^"\\]|\\.)*");$/gm,
          'if (process.env.NODE_ENV !== "production") $1.displayName = $2;',
        )
        .replace(/= Object\.assign\(/g, "= /* @__PURE__ */ Object.assign(");
      const fromReact = [...code.matchAll(/^import \{([^}]*)\} from "react";$/gm)].flatMap((m) => m[1]!.split(",").map((s) => s.trim()));
      const pure = PURE_REACT.filter((name) => fromReact.includes(name));
      const callees = [...pure, ...PURE_FACTORIES];
      out = out.replace(new RegExp(`([=(,]\\s*)\\b(${callees.join("|")})(?=\\s*[<(])`, "g"), "$1/* @__PURE__ */ $2");
      return out === code ? null : { code: out, map: null };
    },
  };
}

function inlineTokenNames() {
  const values = tokens as unknown as Record<string, unknown>;
  return {
    name: "kaeso-inline-token-names",
    enforce: "pre" as const,
    transform(code: string, id: string) {
      if (!id.endsWith("/src/utils/cn.ts")) return null;
      const imported = /^import \{([^}]*)\} from "@logicplanes\/kaeso-tokens";\n/m.exec(code);
      if (!imported) return null;
      const local = new Map(
        imported[1]!
          .split(",")
          .map((s) => s.trim().split(/\s+as\s+/))
          .map(([name, alias]) => [alias ?? name!, values[name!]]),
      );
      const out = code
        .replace(imported[0], "")
        .replace(/Object\.keys\((\w+(?:\.\w+)*)\)|(\w+)\.map\(String\)/g, (whole, keysOf: string | undefined, mapOf: string | undefined) => {
          const [name, ...path] = (keysOf ?? mapOf)!.split(".");
          const value = path.reduce<unknown>((v, key) => (v as Record<string, unknown> | undefined)?.[key], local.get(name!));
          if (value === undefined) throw new Error(`cn.ts: ${whole} reads no token`);
          return JSON.stringify(keysOf ? Object.keys(value as object) : (value as unknown[]).map(String));
        });
      if ([...local.keys()].some((name) => new RegExp(`(?<![\\w."'-])${name}\\b(?!\\s*:)`).test(out)))
        throw new Error("cn.ts: a token is read in a way the build doesn't inline");
      return { code: out, map: null };
    },
  };
}

const CLIENT_LIBRARIES = /^(react|react-dom|@base-ui\/react|@hugeicons\/react|@tanstack\/react-[\w-]+|react-day-picker)(\/|$)/;
const clientModules = new Set<string>();
function clientDirective(): Plugin {
  return {
    name: "kaeso-client-directive",
    buildEnd() {
      const known = new Map<string, boolean>();
      const isClient = (id: string): boolean => {
        const seen = known.get(id);
        if (seen !== undefined) return seen;
        known.set(id, false);
        const info = this.getModuleInfo(id);
        const client = info ? info.importedIds.some((dep) => CLIENT_LIBRARIES.test(dep) || isClient(dep)) : CLIENT_LIBRARIES.test(id);
        known.set(id, client);
        return client;
      };
      clientModules.clear();
      for (const id of this.getModuleIds()) if (isClient(id)) clientModules.add(id);
    },
  };
}

export default defineConfig({
  plugins: [react(), stripVariantDescriptions(), directIcons(), treeShakeable(), inlineTokenNames(), clientDirective()],
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: true,
    minify: true,
    lib: { entry: entries, formats: ["es"] },
    rolldownOptions: {
      external,
      output: {
        banner: (chunk) => {
          const rendered = Object.entries(chunk.modules)
            .filter(([, module]) => module.renderedLength > 0)
            .map(([id]) => id);
          return [chunk.facadeModuleId, ...rendered].some((id) => id && clientModules.has(id)) ? '"use client";' : "";
        },
        entryFileNames: "[name].js",
        chunkFileNames: "chunks/[name]-[hash].js",
        preserveModules: false,
      },
    },
  },
});
