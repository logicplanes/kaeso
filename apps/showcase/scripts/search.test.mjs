import { test } from "node:test";
import assert from "node:assert/strict";
import { matchReason, searchKeywords } from "../src/lib/search.ts";

const button = {
  parts: ["Button.Group"],
  props: ["variant", "loading", "onPressedChange", "pressed"],
  description: "Starts an action. Every button is a pill; exactly one per view is ink.",
  examples: [
    { title: "Danger", description: "The confirm of a destructive flow.", text: "archive project delete key" },
    { title: "Loading", description: "Busy at once; the spinner shows once the work has taken 300ms.", text: "save" },
  ],
};
const GERMAN = {
  "Starts an action. Every button is a pill; exactly one per view is ink.": "Startet eine Aktion. Jeder Button ist eine Pill; genau einer pro Ansicht ist Ink.",
  Danger: "Gefahr",
  "The confirm of a destructive flow.": "Die Bestätigung eines zerstörerischen Ablaufs.",
};
const english = (s) => s;
const german = (s) => GERMAN[s] ?? s;
const named = (pattern) => (label, name) => pattern.replace("{label}", label).replace("{name}", name);
const EN = { part: "Part", prop: "Prop", description: "Description", example: "Example", named: named("{label}: {name}") };
const DE = { part: "Teil", prop: "Prop", description: "Beschreibung", example: "Beispiel", named: named("{label}: {name}") };
const ZH = { part: "子组件", prop: "属性", description: "描述", example: "示例", named: named("{label}：{name}") };

test("a query the name holds needs no reason", () => {
  assert.equal(matchReason("Button", button, "butt", english, EN), null);
  assert.equal(matchReason("Button", button, "  ", english, EN), null);
});

test("a prop or a part says which", () => {
  assert.equal(matchReason("Button", button, "pressedchange", english, EN), "Prop: onPressedChange");
  assert.equal(matchReason("Button", button, "pressed", english, EN), "Prop: pressed");
  assert.equal(matchReason("Button", button, "button.group", english, EN), "Part: Button.Group");
  assert.equal(matchReason("Button", button, "variant", english, ZH), "属性：variant");
});

test("words of the description, in English or the page's language", () => {
  assert.equal(matchReason("Button", button, "exactly one per view", english, EN), "Description");
  assert.equal(matchReason("Button", button, "genau einer pro Ansicht", german, DE), "Beschreibung");
});

test("an example by its title, description or the words it shows, named in the page's language", () => {
  assert.equal(matchReason("Button", button, "destructive", english, EN), "Example: Danger");
  assert.equal(matchReason("Button", button, "zerstörerischen", german, DE), "Beispiel: Gefahr");
  assert.equal(matchReason("Button", button, "delete key", english, EN), "Example: Danger");
  assert.equal(matchReason("Button", button, "button spinner", english, EN), "Example: Loading");
});

test("the place that holds most of the query wins over an earlier kind", () => {
  assert.equal(matchReason("Button", button, "loading spinner", english, EN), "Example: Loading");
  assert.equal(matchReason("Button", button, "loading", english, EN), "Prop: loading");
});

test("a word still being typed names no reason", () => {
  assert.equal(matchReason("Button", button, "va", english, EN), null);
  assert.equal(matchReason("Button", button, "var", english, EN), "Prop: variant");
});

test("nothing that holds the query means no reason", () => {
  assert.equal(matchReason("Button", button, "carousel", english, EN), null);
});

test("the keywords carry every text in both languages, so the palette finds what the reason names", () => {
  const words = searchKeywords(button, german).join(" ");
  for (const w of ["Button.Group", "onPressedChange", "Pill; genau einer", "exactly one per view", "Gefahr", "zerstörerischen", "delete key", "spinner"])
    assert.ok(words.includes(w), w);
});
