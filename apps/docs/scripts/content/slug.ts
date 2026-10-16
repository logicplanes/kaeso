const REMOVE = /[^\p{L}\p{M}\p{N}\p{Pc}\- ]/gu;

export function slugify(text: string): string {
  return text.trim().toLowerCase().replace(REMOVE, "").replace(/ /g, "-");
}

export class Slugger {
  private seen = new Map<string, number>();

  slug(text: string): string {
    const base = slugify(text) || "section";
    let slug = base;
    let n = this.seen.get(base) ?? 0;
    while (this.seen.has(slug)) slug = `${base}-${++n}`;
    this.seen.set(base, n);
    this.seen.set(slug, 0);
    return slug;
  }
}

export function pathSegment(name: string): string {
  return name
    .replace(/\.md$/i, "")
    .trim()
    .toLowerCase()
    .replace(/[\s_]+/g, "-")
    .replace(/[^\p{L}\p{N}-]/gu, "")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

export function humanize(name: string): string {
  const words = name.replace(/[-_]+/g, " ").trim();
  return words.charAt(0).toUpperCase() + words.slice(1);
}
