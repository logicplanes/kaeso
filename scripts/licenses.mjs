import { execFileSync } from "node:child_process";

const ALLOWED = new Set(["MIT", "ISC", "BSD-2-Clause", "BSD-3-Clause", "Apache-2.0", "0BSD"]);
const PUBLISHED = ["@logicplanes/kaeso-tokens", "@logicplanes/kaeso-ui"];

const filters = PUBLISHED.flatMap((name) => ["--filter", name]);
const found = JSON.parse(execFileSync("pnpm", [...filters, "licenses", "list", "--prod", "--json"], { encoding: "utf8" }));
const refused = Object.entries(found).filter(([license]) => !ALLOWED.has(license));
for (const [license, packages] of refused)
  console.error(`licenses: ${packages.map((p) => p.name).join(", ")} ${packages.length === 1 ? "is" : "are"} ${license}`);
if (refused.length) process.exit(1);
const count = Object.values(found).reduce((sum, packages) => sum + packages.length, 0);
console.log(`licenses: the ${count} production dependencies of ${PUBLISHED.join(" and ")} are under ${[...ALLOWED].join(", ")}`);
