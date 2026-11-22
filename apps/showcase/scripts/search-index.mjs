import ts from "typescript";

// Props whose strings people read in an example; everything else in its code (classes, links, handlers) is not a word
// anyone searches the showcase for.
const VISIBLE = new Set(["aria-label", "description", "emptyText", "hint", "label", "message", "name", "placeholder", "text", "title"]);
export const HIDDEN_PROPS = new Set(["className", "data-force", "style"]);
const words = (text) => text.toLowerCase().split(/[^\p{L}\p{N}]+/u);
const norm = (t) => t.replace(/\s+/g, " ").trim();

export function shownWords(code, skip = new Set()) {
  const found = new Set();
  const add = (text) => {
    for (const w of words(text)) if (w.length > 2 && !skip.has(w)) found.add(w);
  };
  const visit = (node) => {
    if (ts.isJsxText(node)) add(node.text);
    else if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
      const owner = node.parent && (ts.isJsxExpression(node.parent) ? node.parent.parent : node.parent);
      const name = owner && (ts.isJsxAttribute(owner) || ts.isPropertyAssignment(owner)) ? owner.name.getText().replace(/^["']|["']$/g, "") : null;
      if (name && VISIBLE.has(name)) add(node.text);
    }
    ts.forEachChild(node, visit);
  };
  visit(ts.createSourceFile("example.tsx", code, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX));
  return [...found].join(" ");
}

/**
 * The search index, and the registry's components no page holds. A page holds what it imports, from any entry, with its
 * parts; an entry shared by many pages (charts, playground) would otherwise lend every page the others' props. What no
 * page imports (a toast page imports the function, not Toaster) goes to the page named after its entry.
 */
export function buildSearchIndex(pages, registry) {
  const all = registry.components.flatMap((e) => e.components.map((c) => ({ entry: e.entry, c })));
  const held = new Map(pages.map((p) => [p.slug, []]));
  const claimed = new Set();
  for (const page of pages) {
    const names = (page.import ?? page.title.replace(/\s+/g, "")).split(",").map((n) => n.trim());
    const entry = page.entry ?? page.slug;
    for (const n of names) {
      const mine = (x) => x.c.name === n || x.c.name.startsWith(`${n}.`);
      const local = all.filter((x) => x.entry === entry && mine(x));
      for (const x of local.length ? local : all.filter(mine)) {
        held.get(page.slug).push(x.c);
        claimed.add(x.c);
      }
    }
  }
  const missing = [];
  for (const x of all) {
    if (claimed.has(x.c)) continue;
    const home = pages.find((p) => p.slug === x.entry) ?? pages.find((p) => (p.entry ?? p.slug) === x.entry);
    if (home) held.get(home.slug).push(x.c);
    else missing.push(`${x.entry}: ${x.c.name}`);
  }
  const index = {};
  for (const page of pages) {
    const main = (page.import ?? page.title.replace(/\s+/g, "")).split(",")[0].trim();
    const own = [...new Set(held.get(page.slug))];
    index[page.slug] = {
      parts: [...new Set(own.map((c) => c.name).filter((n) => n !== main))],
      props: [...new Set(own.flatMap((c) => c.props.filter((p) => !HIDDEN_PROPS.has(p.name) && !p.deprecated).map((p) => p.name)))],
      description: norm(page.description ?? ""),
      examples: page.examples.map((ex) => ({
        title: norm(ex.title),
        ...(ex.description ? { description: norm(ex.description) } : {}),
        text: shownWords(ex.code, new Set(words(`${ex.title} ${ex.description ?? ""}`))),
      })),
    };
  }
  return { index, missing };
}
