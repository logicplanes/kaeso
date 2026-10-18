import { test } from "node:test";
import assert from "node:assert/strict";
import { DOCS_LANG_STORAGE_KEY, docsLangScript } from "../src/lib/languages.ts";

test("the language script passes the page's language and the not-found flag as literals", () => {
  assert.match(docsLangScript("de", false), /\}\)\("de",false\);$/);
  assert.match(docsLangScript("zh-CN", true), /\}\)\("zh-CN",true\);$/);
  assert.equal(docsLangScript("en", false).split(JSON.stringify(DOCS_LANG_STORAGE_KEY)).length, 3);
});

test("the language script cannot close the inline script element it is written into", () => {
  const script = docsLangScript("</script><script>alert(1)</script>", false);
  assert.doesNotMatch(script, /<\/?script/i);
  assert.match(script, /\("\\u003c\/script>\\u003cscript>alert\(1\)\\u003c\/script>",false\);$/);
});
