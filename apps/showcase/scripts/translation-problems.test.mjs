import { test } from "node:test";
import assert from "node:assert/strict";
import { translationProblem } from "./translation-problems.mjs";

test("a text is missing, still English, informal, or fine", () => {
  assert.equal(translationProblem("de", "The button's height.", undefined), "missing");
  assert.equal(translationProblem("de", "The button's height.", "The button's height."), "still the English");
  assert.equal(translationProblem("zh-CN", "Accordion", "Accordion"), null);
  assert.equal(translationProblem("de", "Pass `{ a: 1 }` here.", "Hier `{ a: 1 }` übergeben."), null);
  assert.equal(translationProblem("de", "Your keys.", "Deine Schlüssel."), "says „du“; address the reader with „Sie“");
  assert.equal(translationProblem("de", "Run `du -sh`.", "`du -sh` ausführen."), null);
  assert.equal(translationProblem("zh-CN", "Your keys.", "你的密钥。"), "says 你; address the reader with 您");
  assert.equal(translationProblem("zh-CN", "Your keys.", "您的密钥。"), null);
});
