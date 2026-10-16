import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const assets = join(root, "dist", "assets");
const ALLOWED = new Set(["MIT", "ISC", "BSD-2-Clause", "BSD-3-Clause", "Apache-2.0", "0BSD"]);

function shippedPackages() {
  const packages = new Map();
  for (const file of readdirSync(assets).filter((f) => f.endsWith(".js.map"))) {
    const base = pathToFileURL(join(assets, file));
    for (const source of JSON.parse(readFileSync(join(assets, file), "utf8")).sources) {
      const path = fileURLToPath(new URL(source, base));
      const match = /^(.*[\\/]node_modules[\\/]((?:@[^\\/]+[\\/])?[^\\/]+))[\\/]/.exec(path);
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

function allowed(expression) {
  return expression
    .replace(/[()]/g, "")
    .split(/\s+OR\s+/)
    .some((alternative) => alternative.split(/\s+AND\s+/).every((id) => ALLOWED.has(id.trim())));
}

if (!existsSync(assets)) {
  console.error("licenses: dist/assets is missing; run pnpm build first");
  process.exit(1);
}

const packages = shippedPackages();
const refused = [...packages].map(([name, dir]) => ({ name, license: licenseOf(dir) })).filter((p) => !allowed(p.license));
for (const p of refused) console.error(`licenses: ${p.name} is ${p.license}, which the console can't ship`);
if (refused.length) process.exit(1);
console.log(`licenses: the ${packages.size} packages in the bundle are under ${[...ALLOWED].join(", ")}`);
