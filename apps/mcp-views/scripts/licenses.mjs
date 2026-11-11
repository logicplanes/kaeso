import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { allowed } from "../../console/scripts/licenses.mjs";

const assets = join(dirname(fileURLToPath(import.meta.url)), "..", "dist", "assets");

function shipped() {
  const packages = new Map();
  for (const file of readdirSync(assets).filter((f) => f.endsWith(".js.map"))) {
    const base = pathToFileURL(join(assets, file));
    for (const source of JSON.parse(readFileSync(join(assets, file), "utf8")).sources) {
      const match = /^(.*[\\/]node_modules[\\/]((?:@[^\\/]+[\\/])?[^\\/]+))[\\/]/.exec(fileURLToPath(new URL(source, base)));
      if (match?.[1] && match[2]) packages.set(match[2].replaceAll("\\", "/"), match[1]);
    }
  }
  return packages;
}

function licenseOf(dir) {
  const { license, licenses } = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  if (typeof license === "string") return license;
  if (license?.type) return license.type;
  return licenses?.map((l) => l.type ?? l).join(" OR ") ?? "UNKNOWN";
}

if (!existsSync(assets)) {
  console.error("licenses: dist/assets is missing; run pnpm build first");
  process.exit(1);
}
const packages = shipped();
const refused = [...packages].map(([name, dir]) => ({ name, license: licenseOf(dir) })).filter((p) => !allowed(p.license));
for (const p of refused) console.error(`licenses: ${p.name} is ${p.license}, which the gateway can't ship in its MCP view`);
if (refused.length) process.exit(1);
console.log(`licenses: the ${packages.size} packages in the MCP view are under MIT-compatible licenses`);
