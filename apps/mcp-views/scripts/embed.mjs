import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const built = join(root, "dist", "index.html");
const shipped = join(root, "..", "..", "crates", "kaeso-server", "assets", "mcp-stats.html");

const html = readFileSync(built, "utf8");
if (process.argv.includes("--check")) {
  let current;
  try {
    current = readFileSync(shipped, "utf8");
  } catch {
    current = "";
  }
  if (current !== html) {
    console.error(
      "mcp-views: crates/kaeso-server/assets/mcp-stats.html is not the view this checkout builds; run pnpm --filter kaeso-mcp-views build and commit it",
    );
    process.exit(1);
  }
  console.log("mcp-views: the gateway ships the view this checkout builds");
} else {
  writeFileSync(shipped, html);
  console.log(`mcp-views: wrote crates/kaeso-server/assets/mcp-stats.html (${Math.round(html.length / 1024)} KiB)`);
}
