import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { createHash } from "node:crypto";

// An MCP App is one HTML document that the host writes into a sandboxed frame
// (https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx#ui-resource-format),
// so the script and the styles go inline and nothing is fetched.
function singleDocument(): Plugin {
  return {
    name: "kaeso-single-document",
    apply: "build",
    enforce: "post",
    transformIndexHtml: {
      order: "post",
      handler(html, ctx) {
        const bundle = ctx.bundle;
        if (!bundle) return html;
        let script = "";
        let style = "";
        for (const [name, item] of Object.entries(bundle)) {
          if (item.type === "chunk" && item.isEntry) {
            // With one chunk there is nothing to preload, but Vite leaves the placeholder of the
            // dynamic imports it folded in (the library's translations); its helper takes none as no deps.
            script = item.code.replace(/\n\/\/# sourceMappingURL=\S+\s*$/, "\n").replaceAll("__VITE_PRELOAD__", "void 0");
            if (/__VITE_[A-Z_]+__/.test(script)) throw new Error("kaeso-single-document: the bundle still holds a Vite placeholder");
            Reflect.deleteProperty(bundle, name);
          } else if (item.type === "asset" && name.endsWith(".css")) {
            style = String(item.source).replace(/@font-face\s*\{[^}]*\}/g, "");
            Reflect.deleteProperty(bundle, name);
          } else if (item.type === "asset" && name.endsWith(".woff2")) {
            // The host's default policy has no font-src, so the view uses the system font Kaeso's
            // tokens fall back to instead of shipping fonts it couldn't load.
            Reflect.deleteProperty(bundle, name);
          } else if (item.type === "asset" && !name.endsWith(".map")) {
            throw new Error(`kaeso-single-document: ${name} would be a file of its own; an MCP App resource is one document`);
          }
        }
        if (!script || !style) throw new Error("kaeso-single-document: the entry script or the stylesheet is missing");
        if (/url\((?!["']?data:)/.test(style)) throw new Error("kaeso-single-document: the stylesheet still loads a file");
        const safeScript = script.replaceAll("</script", "<\\/script");
        const hash = createHash("sha256").update(safeScript).digest("base64");
        const csp = [
          "default-src 'none'",
          `script-src 'sha256-${hash}'`,
          "style-src 'unsafe-inline'",
          "img-src data:",
          "font-src 'none'",
          "connect-src 'none'",
          "frame-src 'none'",
          "object-src 'none'",
          "base-uri 'none'",
          "form-action 'none'",
        ].join("; ");
        // Function replacements: the bundle holds `$&` and `$'`, which a replacement string would expand.
        return html
          .replace(/\s*<script type="module" crossorigin src="[^"]+"><\/script>/, () => "")
          .replace(/\s*<link rel="stylesheet" crossorigin href="[^"]+">/, () => "")
          .replace('<meta charset="utf-8" />', () => `<meta charset="utf-8" />\n    <meta http-equiv="Content-Security-Policy" content="${csp}" />`)
          .replace("</head>", () => `  <style>${style.replaceAll("</style", "<\\/style")}</style>\n  </head>`)
          .replace("</body>", () => `  <script type="module">${safeScript}</script>\n  </body>`);
      },
    },
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), singleDocument()],
  build: {
    target: "es2023",
    sourcemap: true,
    cssCodeSplit: false,
    assetsInlineLimit: 0,
    modulePreload: false,
    reportCompressedSize: false,
    rolldownOptions: { output: { codeSplitting: false } },
  },
});
