import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import openapiTS, { astToString } from "openapi-typescript";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const document = join(root, "../../openapi.json");

const spec = JSON.parse(readFileSync(document, "utf8"));
const ast = await openapiTS(spec, { alphabetize: true, exportType: true, rootTypes: false });
const out = join(root, "src/api/generated/schema.ts");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, astToString(ast));
const names = Object.keys(spec.components?.schemas ?? {}).sort();
writeFileSync(
  join(root, "src/api/generated/types.ts"),
  `import type { components } from "./schema";\n\ntype S = components["schemas"];\n\n${names.map((n) => `export type ${n} = S["${n}"];`).join("\n")}\n`,
);
console.log(
  `gen:api: wrote src/api/generated/schema.ts from openapi.json (${Object.keys(spec.paths).length} paths, ${Object.keys(spec.components?.schemas ?? {}).length} schemas)`,
);
