import { defineConfig } from "astro/config";
import react from "@astrojs/react";
import mdx from "@astrojs/mdx";
import { unified } from "@astrojs/markdown-remark";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";
import { localizedHeadingIds } from "./scripts/localized-heading-ids.mjs";
import { folderIndexes } from "./scripts/folder-indexes.mjs";
import { sitemap } from "./scripts/sitemap.mjs";
import { contentSecurityPolicy } from "./scripts/content-security-policy.mjs";
import { refuseRawMarkup } from "./scripts/refuse-raw-markup.mjs";

const ui = fileURLToPath(new URL("../../packages/ui/src", import.meta.url));

export default defineConfig({
  site: "https://ui.kaeso.app",
  output: "static",
  trailingSlash: "never",
  build: { format: "file" },
  integrations: [react(), mdx(), folderIndexes(), sitemap(), contentSecurityPolicy()],
  markdown: {
    syntaxHighlight: false,
    processor: unified({ remarkPlugins: [refuseRawMarkup], rehypePlugins: [localizedHeadingIds] }),
  },
  devToolbar: { enabled: false },
  prefetch: { prefetchAll: true, defaultStrategy: "hover" },
  vite: {
    plugins: [tailwindcss()],
    resolve: {
      alias: [
        { find: /^@logicplanes\/kaeso-ui$/, replacement: `${ui}/index.ts` },
        { find: /^@logicplanes\/kaeso-ui\/format$/, replacement: `${ui}/format/index.ts` },
        { find: /^@logicplanes\/kaeso-ui\/i18n$/, replacement: `${ui}/i18n/index.tsx` },
        { find: /^@logicplanes\/kaeso-ui\/(markdown|playground)$/, replacement: `${ui}/components/$1/index.ts` },
        { find: /^@logicplanes\/kaeso-ui\/blocks\/(.*)$/, replacement: `${ui}/blocks/$1/index.ts` },
        { find: /^@logicplanes\/kaeso-ui\/styles\/(.*)$/, replacement: `${ui}/styles/$1` },
        { find: /^~\/(.*)$/, replacement: fileURLToPath(new URL("./src/$1", import.meta.url)) },
      ],
    },
  },
});
