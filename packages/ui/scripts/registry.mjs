import { readdirSync, existsSync, writeFileSync, writeSync, mkdirSync, readFileSync } from "node:fs";
import { join, dirname, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import docgen from "react-docgen-typescript";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(root, "src");
const groups = [
  ["components", join(src, "components")],
  ["blocks", join(src, "blocks")],
];

const files = [];
const entries = [];
for (const [kind, dir] of groups) {
  for (const name of readdirSync(dir)) {
    const d = join(dir, name);
    if (name.startsWith("_") || !existsSync(join(d, "index.ts"))) continue;
    entries.push({ kind, entry: name, dir: d });
    const exported = new Set([...readFileSync(join(d, "index.ts"), "utf8").matchAll(/from "\.\/([\w-]+)\.js"/g)].map((m) => `${m[1]}.tsx`));
    for (const f of readdirSync(d)) {
      if (f.endsWith(".tsx") && !f.includes(".test.") && exported.has(f)) files.push({ kind, entry: name, file: join(d, f) });
    }
  }
}

const parser = docgen.withCustomConfig(join(root, "tsconfig.json"), {
  savePropValueAsString: true,
  shouldExtractLiteralValuesFromEnum: true,
  shouldRemoveUndefinedFromOptional: true,
  skipChildrenPropWithoutDoc: false,
  propFilter: (prop) => {
    if (!prop.parent) return !prop.declarations?.some((d) => d.fileName.includes("node_modules"));
    return !prop.parent.fileName.includes("node_modules");
  },
});
const parsed = parser.parse(files.map((f) => f.file));

const missingDocs = entries.filter((e) => !existsSync(join(e.dir, "docs.ts"))).map((e) => `${relative(root, e.dir)}/docs.ts is missing`);
if (missingDocs.length) fail(missingDocs);

const dataScript = `(async () => {
const entries = ${JSON.stringify(entries.map((e) => `${e.kind}/${e.entry}`))};
const out = { variants: {}, docs: {}, common: (await import(${JSON.stringify(join(src, "component-docs.ts"))})).COMMON_PROP_DOCS };
for (const e of entries) {
  out.docs[e] = (await import(${JSON.stringify(`${src}/`)} + e + "/docs.ts")).docs;
  const m = await import(${JSON.stringify(`${src}/`)} + e + "/index.ts");
  for (const [k, v] of Object.entries(m)) {
    const match = /^KAESO_(.+)_(VARIANTS|DEFAULT_VARIANTS)$/.exec(k);
    if (!match) continue;
    out.variants[match[1]] ??= { entry: e };
    if (match[2] === "VARIANTS") out.variants[match[1]].variants = Object.fromEntries(Object.entries(v).map(([axis, keys]) => [axis, Object.fromEntries(Object.entries(keys).map(([key, val]) => [key, val.description]))]));
    else out.variants[match[1]].defaults = v;
  }
}
process.stdout.write(JSON.stringify(out));
})();
`;
const tsx = join(root, "node_modules/.bin/tsx");
const data = JSON.parse(execFileSync(tsx, ["--eval", dataScript], { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 }));

function fail(problems) {
  writeSync(2, `registry: ${problems.length} problem${problems.length === 1 ? "" : "s"}:\n  ${problems.join("\n  ")}\n`);
  process.exit(1);
}

function isComponent(doc, file) {
  if (/^[A-Z0-9_]+$/.test(doc.displayName) || doc.displayName.endsWith("Context") || !/^[A-Z]/.test(doc.displayName)) return false;
  if (doc.displayName.includes(".")) return true;
  return new RegExp(`^export (?:const|function) ${doc.displayName}\\b`, "m").test(readFileSync(file, "utf8"));
}

const problems = [];
const byEntry = new Map();
const documented = new Set();
for (const d of parsed) {
  const f = files.find((x) => x.file === d.filePath);
  if (!f || !isComponent(d, f.file)) continue;
  const key = `${f.kind}/${f.entry}`;
  const where = `${relative(root, join(src, key, "docs.ts"))}: ${d.displayName}`;
  const doc = data.docs[key]?.[d.displayName];
  documented.add(`${key}/${d.displayName}`);
  if (!doc) {
    problems.push(`${where} has no entry`);
    continue;
  }
  if (!doc.description?.trim()) problems.push(`${where} has no description`);
  const names = new Set(Object.keys(d.props));
  for (const name of [...Object.keys(doc.props ?? {}), ...Object.keys(doc.deprecated ?? {})]) {
    if (!names.has(name)) problems.push(`${where} describes ${name}, which is not one of its props`);
  }
  const props = Object.values(d.props)
    .map((p) => {
      const deprecated = doc.deprecated?.[p.name];
      const description = deprecated ?? doc.props?.[p.name] ?? data.common[p.name];
      if (!description?.trim()) problems.push(`${where} has no description for its prop ${p.name}`);
      return {
        name: p.name,
        type: p.type.raw ?? p.type.name,
        required: p.required,
        default: p.defaultValue?.value ?? null,
        description: description ?? "",
        ...(deprecated ? { deprecated: true } : {}),
      };
    })
    .sort((a, b) => Number(b.required) - Number(a.required) || a.name.localeCompare(b.name));
  if (!byEntry.has(key)) byEntry.set(key, []);
  byEntry.get(key).push({ name: d.displayName, description: doc.description, props, file: relative(root, d.filePath) });
}
for (const [key, docs] of Object.entries(data.docs)) {
  for (const name of Object.keys(docs)) {
    if (!documented.has(`${key}/${name}`)) problems.push(`${relative(root, join(src, key, "docs.ts"))}: ${name} is not a component of this entry`);
  }
}
if (problems.length) fail(problems);

const components = [];
for (const [key, list] of byEntry) {
  const [kind, entry] = key.split("/");
  const v = Object.entries(data.variants).filter(([, x]) => x.entry === key);
  components.push({
    entry,
    kind,
    import: kind === "blocks" ? `@logicplanes/kaeso-ui/blocks/${entry}` : `@logicplanes/kaeso-ui/${entry}`,
    components: list.sort((a, b) => a.name.localeCompare(b.name)),
    variants: Object.fromEntries(v.map(([name, x]) => [name, { axes: x.variants ?? {}, defaults: x.defaults ?? {} }])),
  });
}
components.sort((a, b) => a.entry.localeCompare(b.entry));

const registryFile = join(root, "ai/component-registry.json");
if (existsSync(registryFile)) {
  const now = new Map(components.flatMap((e) => e.components.map((c) => [`${e.entry}/${c.name}`, c])));
  const lost = [];
  for (const e of JSON.parse(readFileSync(registryFile, "utf8")).components ?? []) {
    for (const c of e.components) {
      const source = join(root, c.file);
      const stillExported =
        existsSync(source) && (c.name.includes(".") || new RegExp(`^export (?:const|function) ${c.name}\\b`, "m").test(readFileSync(source, "utf8")));
      if (!stillExported) continue;
      const current = now.get(`${e.entry}/${c.name}`);
      if (!current) lost.push(`${c.file}: ${c.name} is no longer parsed`);
      else if (c.props.length && !current.props.length) lost.push(`${c.file}: ${c.name} lost its props`);
    }
  }
  if (lost.length) fail(lost.map((l) => `the parser lost a component that is still exported: ${l}`));
}

const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
mkdirSync(join(root, "ai"), { recursive: true });
writeFileSync(registryFile, `${JSON.stringify({ package: pkg.name, version: pkg.version, components }, null, 2)}\n`);

const cell = (text) => String(text).replace(/\|/g, "\\|").replace(/\n/g, " ");
const md = [
  `# ${pkg.name} component registry`,
  "",
  `Version ${pkg.version}. Generated from the component types, their descriptions and variants; do not edit.`,
  "",
];
for (const c of components) {
  const names = [...new Set(c.components.map((x) => x.name.split(".")[0]))];
  md.push(`## ${c.entry}`, "", `\`import { ${names.join(", ")} } from "${c.import}";\``, "");
  for (const [name, v] of Object.entries(c.variants)) {
    for (const [axis, keys] of Object.entries(v.axes)) {
      if (!Object.keys(keys).length) continue;
      const values = Object.entries(keys).map(([k, d]) => `\`${k}\`${v.defaults?.[axis] === k ? " (default)" : ""} — ${d}`);
      md.push(`- **${axis}** (${name.toLowerCase()}): ${values.join("; ")}`);
    }
  }
  for (const comp of c.components) {
    md.push("", `### ${comp.name}`, "", comp.description, "");
    if (!comp.props.length) continue;
    md.push("| Prop | Type | Default | Description |", "|---|---|---|---|");
    for (const p of comp.props) {
      const description = p.deprecated ? `Deprecated. ${p.description}` : p.description;
      md.push(`| \`${p.name}\`${p.required ? " *" : ""} | \`${cell(p.type)}\` | ${p.default ? `\`${p.default}\`` : ""} | ${cell(description)} |`);
    }
  }
  md.push("");
}
writeFileSync(join(root, "ai/component-registry.md"), `${md.join("\n").replace(/\n{3,}/g, "\n\n")}\n`);
console.log(`registry: ${components.length} entries, ${byEntry.size ? [...byEntry.values()].flat().length : 0} components`);
