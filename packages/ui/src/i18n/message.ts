export type MessageValues = Record<string, string | number>;

function branches(src: string): Map<string, string> {
  const out = new Map<string, string>();
  let i = 0;
  while (i < src.length) {
    while (i < src.length && /\s/.test(src[i] ?? "")) i++;
    let key = "";
    while (i < src.length && src[i] !== "{" && !/\s/.test(src[i] ?? "")) key += src[i++];
    while (i < src.length && src[i] !== "{") i++;
    if (i >= src.length) break;
    let depth = 0;
    const start = ++i;
    for (; i < src.length; i++) {
      if (src[i] === "{") depth++;
      else if (src[i] === "}") {
        if (depth === 0) break;
        depth--;
      }
    }
    out.set(key, src.slice(start, i));
    i++;
  }
  return out;
}

export function formatMessage(template: string, values: MessageValues | undefined, locale: string, number?: number): string {
  if (!template.includes("{") && !(number !== undefined && template.includes("#"))) return template;
  let out = "";
  let i = 0;
  while (i < template.length) {
    const c = template[i];
    if (c === "#" && number !== undefined) {
      out += new Intl.NumberFormat(locale).format(number);
      i++;
      continue;
    }
    if (c !== "{") {
      out += c;
      i++;
      continue;
    }
    let depth = 0;
    let j = i;
    for (; j < template.length; j++) {
      if (template[j] === "{") depth++;
      else if (template[j] === "}" && --depth === 0) break;
    }
    const inner = template.slice(i + 1, j);
    const [name = "", kind, ...rest] = inner.split(",");
    const key = name.trim();
    const value = values?.[key];
    if (kind === undefined) {
      out += value === undefined ? `{${key}}` : typeof value === "number" ? new Intl.NumberFormat(locale).format(value) : value;
    } else {
      const options = branches(rest.join(","));
      const type = kind.trim();
      let chosen: string | undefined;
      if (type === "plural" && typeof value === "number") {
        chosen = options.get(`=${value}`) ?? options.get(new Intl.PluralRules(locale).select(value)) ?? options.get("other");
        out += formatMessage(chosen ?? "", values, locale, value);
      } else {
        chosen = options.get(String(value)) ?? options.get("other");
        out += formatMessage(chosen ?? "", values, locale);
      }
    }
    i = j + 1;
  }
  return out;
}
