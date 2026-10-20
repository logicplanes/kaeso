import { createHash } from "node:crypto";
import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative, sep } from "node:path";

export const MAX_HEADER_BYTES = 4096;

const DIRECTIVES = [
  ["default-src", "'self'"],
  ["script-src", "'self'"],
  ["style-src", "'self'"],
  ["img-src", "'self'", "data:"],
  ["font-src", "'self'"],
  ["connect-src", "'self'"],
  ["object-src", "'none'"],
  ["base-uri", "'self'"],
  ["form-action", "'self'"],
  ["frame-ancestors", "'none'"],
];
const HEADER_ONLY = new Set(["frame-ancestors"]);
const SCRIPT_TYPES = new Set([
  "",
  "module",
  "importmap",
  "speculationrules",
  "text/javascript",
  "application/javascript",
  "application/ecmascript",
  "text/ecmascript",
]);
const URL_ATTRIBUTES = new Set(["href", "src", "action", "formaction", "xlink:href", "data"]);
const RAW_TEXT = new Set(["script", "style"]);
const ENTITIES = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", colon: ":", tab: "\t", newline: "\n" };
const META = /<meta\s+http-equiv="content-security-policy"[^>]*>/gi;

const decode = (s) =>
  s.replace(/&(#\d+|#x[\da-f]+|\w+);?/gi, (whole, name) => {
    if (name[0] !== "#") return ENTITIES[name.toLowerCase()] ?? whole;
    return String.fromCodePoint(name[1] === "x" || name[1] === "X" ? parseInt(name.slice(2), 16) : Number(name.slice(1)));
  });

function readTag(html, start) {
  const name = /^[a-zA-Z][\w:-]*/.exec(html.slice(start + 1, start + 64))?.[0];
  if (!name) return null;
  const attributes = [];
  let i = start + 1 + name.length;
  while (i < html.length) {
    while (i < html.length && /\s/.test(html[i])) i++;
    if (i >= html.length || html[i] === ">") break;
    if (html[i] === "/") {
      i++;
      continue;
    }
    const attribute = /^[^\s/>=]+/.exec(html.slice(i))?.[0] ?? html[i];
    i += attribute.length;
    let value = "";
    const equals = /^\s*=\s*/.exec(html.slice(i));
    if (equals) {
      i += equals[0].length;
      const quote = html[i];
      if (quote === '"' || quote === "'") {
        const end = html.indexOf(quote, i + 1);
        value = html.slice(i + 1, end === -1 ? html.length : end);
        i = end === -1 ? html.length : end + 1;
      } else {
        value = /^[^\s>]*/.exec(html.slice(i))?.[0] ?? "";
        i += value.length;
      }
    }
    attributes.push([attribute.toLowerCase(), decode(value)]);
  }
  const end = html.indexOf(">", i);
  return { name: name.toLowerCase(), attributes, end: end === -1 ? html.length : end + 1 };
}

export function inlineContent(html) {
  const found = { scripts: [], styles: [], styleAttributes: [], handlers: [], javascriptUrls: [] };
  let i = 0;
  while ((i = html.indexOf("<", i)) !== -1) {
    if (html.startsWith("<!--", i)) {
      const end = html.indexOf("-->", i + 4);
      i = end === -1 ? html.length : end + 3;
      continue;
    }
    const tag = /[a-zA-Z]/.test(html[i + 1] ?? "") ? readTag(html, i) : null;
    if (!tag) {
      const end = html.indexOf(">", i + 1);
      i = end === -1 ? html.length : end + 1;
      continue;
    }
    i = tag.end;
    const attributes = new Map(tag.attributes);
    for (const [name, value] of tag.attributes) {
      if (name === "style") found.styleAttributes.push(value);
      else if (/^on[a-z]+$/.test(name)) found.handlers.push(`${name} on <${tag.name}>`);
      else if (URL_ATTRIBUTES.has(name) && /^javascript:/i.test(value.replace(/[\s\u0000-\u001f]/g, ""))) found.javascriptUrls.push(`${name} on <${tag.name}>`);
    }
    if (!RAW_TEXT.has(tag.name)) continue;
    const close = new RegExp(`</${tag.name}[\\s/>]`, "ig");
    close.lastIndex = i;
    const end = close.exec(html)?.index ?? html.length;
    const text = html.slice(i, end).replace(/\r\n?/g, "\n");
    i = end;
    if (tag.name === "style") found.styles.push(text);
    else if (!attributes.has("src") && SCRIPT_TYPES.has((attributes.get("type") ?? "").trim().toLowerCase())) found.scripts.push(text);
  }
  return found;
}

export const hashOf = (text) => `'sha256-${createHash("sha256").update(text, "utf8").digest("base64")}'`;

function htmlFiles(dir, base = dir) {
  return readdirSync(dir)
    .flatMap((name) => {
      const full = join(dir, name);
      if (statSync(full).isDirectory()) return htmlFiles(full, base);
      return name.endsWith(".html") ? [relative(base, full).split(sep).join("/")] : [];
    })
    .sort();
}

function render(scriptHashes, styleHashes, styleAttributes, { header }) {
  const extra = { "script-src": scriptHashes, "style-src": styleHashes };
  const directives = DIRECTIVES.filter(([name]) => header || !HEADER_ONLY.has(name)).map(([name, ...sources]) => [name, ...sources, ...(extra[name] ?? [])]);
  if (styleAttributes) directives.splice(directives.findIndex(([name]) => name === "style-src") + 1, 0, ["style-src-attr", "'unsafe-inline'"]);
  return directives.map((parts) => parts.join(" ")).join("; ");
}

export function sitePolicy(dir, { hashes = false, styleAttributes = false } = {}) {
  const problems = [];
  const scripts = new Set();
  const styles = new Set();
  let attributes = 0;
  const files = htmlFiles(dir);
  for (const file of files) {
    const found = inlineContent(readFileSync(join(dir, file), "utf8"));
    for (const handler of found.handlers) problems.push(`${file}: an inline event handler, ${handler}; attach it from a script file`);
    for (const url of found.javascriptUrls) problems.push(`${file}: a javascript: URL in ${url}; use a script file`);
    if (!hashes && found.scripts.length) problems.push(`${file}: ${found.scripts.length} inline <script>; load it from a file`);
    if (!hashes && found.styles.length) problems.push(`${file}: ${found.styles.length} inline <style>; load it from a stylesheet`);
    if (!styleAttributes && found.styleAttributes.length)
      problems.push(`${file}: ${found.styleAttributes.length} style attributes, the first style="${found.styleAttributes[0]}"; use a class`);
    for (const text of found.scripts) scripts.add(hashOf(text));
    for (const text of found.styles) styles.add(hashOf(text));
    attributes += found.styleAttributes.length;
  }
  const scriptHashes = [...scripts].sort();
  const styleHashes = [...styles].sort();
  const allowAttributes = styleAttributes && attributes > 0;
  const header = render(scriptHashes, styleHashes, allowAttributes, { header: true });
  const meta = render(scriptHashes, styleHashes, allowAttributes, { header: false });
  if (Buffer.byteLength(header) > MAX_HEADER_BYTES)
    problems.push(`the policy is ${Buffer.byteLength(header)} bytes, over ${MAX_HEADER_BYTES}; move inline scripts and styles to files`);
  return { header, meta, problems, files, scripts: scriptHashes.length, styles: styleHashes.length };
}

export function withPolicyMeta(html, policy) {
  const cleaned = html.replace(META, "");
  const charset = /<meta\s+charset="?utf-8"?\s*\/?>/i.exec(cleaned);
  if (!charset) return { problem: `has no <meta charset="utf-8"> to put the policy after` };
  if (/<(script|style|link)\b/i.test(cleaned.slice(0, charset.index)))
    return { problem: "loads a script or a stylesheet before <meta charset>, where the policy doesn't reach" };
  const at = charset.index + charset[0].length;
  return { html: `${cleaned.slice(0, at)}<meta http-equiv="content-security-policy" content="${policy}">${cleaned.slice(at)}` };
}

const SITE_HEADERS = [
  ["Strict-Transport-Security", "max-age=63072000; includeSubDomains"],
  ["X-Content-Type-Options", "nosniff"],
  ["Referrer-Policy", "strict-origin-when-cross-origin"],
];

function headersFile(policy) {
  const lines = [["Content-Security-Policy", policy], ...SITE_HEADERS].map(([name, value]) => `  ${name}: ${value}\n`);
  return `/*\n${lines.join("")}`;
}

export function writeSitePolicy(dir, options = {}) {
  const policy = sitePolicy(dir, options);
  const problems = [...policy.problems];
  const pages = [];
  for (const file of policy.files) {
    const result = withPolicyMeta(readFileSync(join(dir, file), "utf8"), policy.meta);
    if (result.problem) problems.push(`${file}: ${result.problem}`);
    else pages.push([file, result.html]);
  }
  if (problems.length) throw new Error(`csp: ${problems.length} to fix in ${dir}\n  ${problems.join("\n  ")}`);
  for (const [file, html] of pages) writeFileSync(join(dir, file), html);
  writeFileSync(join(dir, "csp.txt"), `${policy.header}\n`);
  writeFileSync(join(dir, "_headers"), headersFile(policy.header));
  return `csp: ${policy.files.length} pages under one policy, ${policy.scripts} inline script and ${policy.styles} inline style hashes, in csp.txt and _headers`;
}
