import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const ALLOWED = new Set(["MIT", "ISC", "BSD-2-Clause", "BSD-3-Clause", "Apache-2.0", "0BSD"]);

export interface ShippedPackage {
  name: string;
  version: string;
  license: string;
}

export interface LicenseReport {
  packages: ShippedPackage[];
  refused: ShippedPackage[];
  lines: string[];
}

const mapsIn = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const path = join(dir, e.name);
    if (e.isDirectory()) return mapsIn(path);
    return e.name.endsWith(".map") ? [path] : [];
  });

export function packageRoot(source: string): string | null {
  const parts = source.split(sep);
  const last = parts.lastIndexOf("node_modules");
  if (last < 0) return null;
  const name = parts[last + 1] ?? "";
  const length = name.startsWith("@") ? 3 : 2;
  return parts.slice(0, last + length).join(sep);
}

export function allowed(expression: string): boolean {
  const spdx = expression.trim().replace(/^\((.*)\)$/, "$1");
  if (/\s+OR\s+/.test(spdx)) return spdx.split(/\s+OR\s+/).some(allowed);
  if (/\s+AND\s+/.test(spdx)) return spdx.split(/\s+AND\s+/).every(allowed);
  return ALLOWED.has(spdx);
}

function licenseOf(manifest: { license?: unknown; licenses?: unknown }): string {
  if (typeof manifest.license === "string") return manifest.license;
  if (manifest.license && typeof manifest.license === "object" && "type" in manifest.license) return String(manifest.license.type);
  if (Array.isArray(manifest.licenses)) return manifest.licenses.map((l: { type?: string }) => l.type ?? "").join(" OR ");
  return "UNKNOWN";
}

export function shippedLicenses(dist: string): LicenseReport {
  const roots = new Set<string>();
  for (const map of mapsIn(dist)) {
    const { sources = [] } = JSON.parse(readFileSync(map, "utf8")) as { sources?: string[] };
    for (const source of sources) {
      const root = packageRoot(resolve(dirname(map), source));
      if (root) roots.add(root);
    }
  }
  const byName = new Map<string, ShippedPackage>();
  for (const root of roots) {
    const file = join(root, "package.json");
    if (!existsSync(file)) continue;
    const manifest = JSON.parse(readFileSync(file, "utf8")) as { name?: string; version?: string; license?: unknown; licenses?: unknown };
    const name = manifest.name ?? root;
    byName.set(`${name}@${manifest.version ?? ""}`, { name, version: manifest.version ?? "", license: licenseOf(manifest) });
  }
  const packages = [...byName.values()].sort((a, b) => a.name.localeCompare(b.name));
  const refused = packages.filter((p) => !allowed(p.license));
  const counts = new Map<string, number>();
  for (const p of packages) counts.set(p.license, (counts.get(p.license) ?? 0) + 1);
  const lines = [
    `licenses: ${packages.length} packages in the build: ${[...counts].map(([l, n]) => `${l} ${n}`).join(", ")}`,
    ...refused.map((p) => `licenses: ${p.name}@${p.version} is ${p.license}, which is not allowed (${[...ALLOWED].join(", ")})`),
  ];
  return { packages, refused, lines };
}

if (import.meta.main) {
  const report = shippedLicenses(join(dirname(fileURLToPath(import.meta.url)), "../dist"));
  for (const line of report.lines) (report.refused.length ? console.error : console.log)(line);
  process.exitCode = report.refused.length || !report.packages.length ? 1 : 0;
}
