/**
 * Where a component page's View source points, relative to the repository: the page's own `source`, else the file the
 * registry says its main component is written in (several components share a folder, such as dialog/ or kaeso/), else
 * its entry's folder.
 */
export function sourcePathOf(slug, page, registry) {
  if (page.source) return `packages/ui/src/${page.source}`;
  const entry = page.entry ?? slug;
  const main = (page.import ?? page.title.replace(/\s+/g, "")).split(",")[0].trim();
  const file = registry.components.find((c) => c.entry === entry)?.components.find((c) => c.name === main)?.file;
  return file ? `packages/ui/${file}` : `packages/ui/src/components/${entry}`;
}

/** "slug: path" for every page whose source the checkout doesn't have. */
export function missingSources(sources, exists) {
  return Object.entries(sources)
    .filter(([, path]) => !exists(path))
    .map(([slug, path]) => `${slug}: ${path}`);
}
