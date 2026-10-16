const LOCALIZED = /\/src\/guides\/[^/]+\/(de|zh-CN)\.mdx?$/;

const textOf = (node) => (node.type === "text" ? node.value : (node.children ?? []).map(textOf).join(""));

const slug = (text) =>
  text
    .toLowerCase()
    .trim()
    .replace(/[^\p{L}\p{N}\s-]/gu, "")
    .replace(/\s+/g, "-");

export function localizedHeadingIds() {
  return (tree, file) => {
    const lang = LOCALIZED.exec(file.path ?? "")?.[1];
    if (!lang) return;
    const seen = new Map();
    const visit = (node) => {
      if (node.type === "element" && /^h[1-6]$/.test(node.tagName) && !node.properties?.id) {
        const base = `${lang.toLowerCase()}-${slug(textOf(node))}`;
        const count = seen.get(base) ?? 0;
        seen.set(base, count + 1);
        node.properties = { ...node.properties, id: count ? `${base}-${count}` : base };
      }
      for (const child of node.children ?? []) visit(child);
    };
    visit(tree);
  };
}
