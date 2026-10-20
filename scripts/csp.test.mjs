import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, test } from "node:test";
import { MAX_HEADER_BYTES, hashOf, inlineContent, sitePolicy, withPolicyMeta, writeSitePolicy } from "./csp.mjs";

const STRICT =
  "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'";
const page = (head = "", body = "") => `<!doctype html><html><head><meta charset="utf-8" />${head}</head><body>${body}</body></html>`;
let dir = "";
afterEach(() => dir && rmSync(dir, { recursive: true, force: true }));

function site(files) {
  dir = mkdtempSync(join(tmpdir(), "csp-"));
  for (const [file, html] of Object.entries(files)) {
    mkdirSync(join(dir, file, ".."), { recursive: true });
    writeFileSync(join(dir, file), html);
  }
  return dir;
}

test("a hash is the base64 SHA-256 of the script's text, as browsers compute it", () => {
  const text = "document.documentElement.dataset.theme = 'dark';";
  assert.equal(hashOf(text), `'sha256-${createHash("sha256").update(text).digest("base64")}'`);
});

test("only executable inline scripts count, not scripts with a source, data blocks or markup inside attributes and comments", () => {
  const found = inlineContent(
    page(
      `<script src="/app.js"></script><script type="application/ld+json">{"@type":"WebSite"}</script><script>one()</script><script type="module">two()</script>`,
      `<astro-island props='{"code":"<script>three()</script>"}'></astro-island><!-- <script>four()</script> --><SCRIPT>five()</SCRIPT >`,
    ),
  );
  assert.deepEqual(found.scripts, ["one()", "two()", "five()"]);
});

test("inline style elements, style attributes, event handlers and javascript: URLs are all found", () => {
  const found = inlineContent(
    page(
      `<style>p{margin:0}</style>`,
      `<p style="color:var(--k-text)">a</p><button onclick="go()">b</button><a href=" jav&#x61;script:go()">c</a><img src=x onerror=go()>`,
    ),
  );
  assert.deepEqual(found.styles, ["p{margin:0}"]);
  assert.deepEqual(found.styleAttributes, ["color:var(--k-text)"]);
  assert.deepEqual(found.handlers, ["onclick on <button>", "onerror on <img>"]);
  assert.deepEqual(found.javascriptUrls, ["href on <a>"]);
});

test("a site without inline code gets the strict policy, and the policy without frame-ancestors in its pages", () => {
  const out = writeSitePolicy(site({ "index.html": page(`<script src="/app.js"></script>`), "de/guides.html": page() }));
  assert.match(out, /^csp: 2 pages under one policy, 0 inline script and 0 inline style hashes/);
  assert.equal(readFileSync(join(dir, "csp.txt"), "utf8"), `${STRICT}\n`);
  assert.equal(
    readFileSync(join(dir, "_headers"), "utf8"),
    `/*\n  Content-Security-Policy: ${STRICT}\n  Strict-Transport-Security: max-age=63072000; includeSubDomains\n  X-Content-Type-Options: nosniff\n  Referrer-Policy: strict-origin-when-cross-origin\n`,
  );
  const html = readFileSync(join(dir, "de/guides.html"), "utf8");
  assert.ok(html.includes(`<meta charset="utf-8" /><meta http-equiv="content-security-policy" content="${STRICT.replace("; frame-ancestors 'none'", "")}">`));
});

test("a strict site fails the build on any inline script, style or style attribute, naming the page", () => {
  site({ "a.html": page(`<script>x()</script>`), "b.html": page(`<style>p{}</style>`), "c.html": page("", `<p style="margin:0">c</p>`) });
  assert.throws(
    () => writeSitePolicy(dir),
    (e) =>
      /csp: 3 to fix/.test(e.message) &&
      e.message.includes("a.html: 1 inline <script>; load it from a file") &&
      e.message.includes("b.html: 1 inline <style>; load it from a stylesheet") &&
      e.message.includes(`c.html: 1 style attributes, the first style="margin:0"; use a class`),
  );
  assert.equal(readFileSync(join(dir, "a.html"), "utf8").includes("http-equiv"), false);
});

test("a site that allows hashes lists each distinct inline script and style once, and style attributes only when it has them", () => {
  site({ "a.html": page(`<script>x()</script><style>p{}</style>`), "b.html": page(`<script>x()</script><script>y()</script>`, `<p style="margin:0">b</p>`) });
  const policy = sitePolicy(dir, { hashes: true, styleAttributes: true });
  assert.deepEqual(policy.problems, []);
  assert.equal(policy.scripts, 2);
  assert.ok(policy.header.includes(`script-src 'self' ${[hashOf("x()"), hashOf("y()")].sort().join(" ")};`));
  assert.ok(policy.header.includes(`style-src 'self' ${hashOf("p{}")}; style-src-attr 'unsafe-inline';`));
  const without = sitePolicy(site({ "a.html": page(`<script>x()</script>`) }), { hashes: true, styleAttributes: true });
  assert.equal(without.header.includes("style-src-attr"), false);
});

test("event handlers and javascript: URLs fail the build even where hashes are allowed", () => {
  site({ "a.html": page("", `<button onclick="go()">a</button><a href="javascript:go()">b</a>`) });
  assert.throws(
    () => writeSitePolicy(dir, { hashes: true, styleAttributes: true }),
    (e) => e.message.includes("a.html: an inline event handler, onclick on <button>") && e.message.includes("a.html: a javascript: URL in href on <a>"),
  );
});

test("a policy too long for a response header fails the build", () => {
  const scripts = Array.from({ length: 80 }, (_, i) => `<script>s${i}()</script>`).join("");
  const policy = sitePolicy(site({ "a.html": page(scripts) }), { hashes: true });
  assert.match(policy.problems.join("\n"), new RegExp(`the policy is \\d+ bytes, over ${MAX_HEADER_BYTES}`));
});

test("the policy goes right after the charset, replaces an older one, and refuses a page that loads code before it", () => {
  const once = withPolicyMeta(page(), "default-src 'self'").html ?? "";
  const twice = withPolicyMeta(once, "default-src 'none'").html ?? "";
  assert.equal(twice.match(/http-equiv/g)?.length, 1);
  assert.ok(twice.includes(`<meta charset="utf-8" /><meta http-equiv="content-security-policy" content="default-src 'none'">`));
  assert.match(withPolicyMeta(`<html><head><script src="/a.js"></script><meta charset="utf-8"></head></html>`, "x").problem ?? "", /before <meta charset>/);
  assert.match(withPolicyMeta(`<html><head></head></html>`, "x").problem ?? "", /no <meta charset/);
});
