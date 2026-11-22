import { test } from "node:test";
import assert from "node:assert/strict";
import { buildSearchIndex, shownWords } from "./search-index.mjs";

test("an example's shown words are its text and readable props, never its code", () => {
  const code = `<Field label="Monthly budget" hint="Resets on the first">
  <Input placeholder="Northwind Labs" className="w-full mono" href="/keys/archive" onChange={() => track("spend")} />
  <Button aria-label="Rotate secret">Archive project</Button>
</Field>`;
  const words = shownWords(code).split(" ");
  for (const w of ["monthly", "budget", "resets", "first", "northwind", "labs", "rotate", "secret", "archive", "project"]) assert.ok(words.includes(w), w);
  for (const w of ["field", "input", "button", "full", "mono", "keys", "onchange", "track", "spend"]) assert.ok(!words.includes(w), w);
});

test("words already in the example's title or description, and short words, are left out", () => {
  assert.equal(shownWords(`<Button>Delete the key</Button>`, new Set(["delete"])), "the key");
  assert.equal(shownWords(`<Badge>On</Badge>`), "");
});

const registry = {
  components: [
    { entry: "charts", components: [comp("BarChart", ["bars"]), comp("Donut", ["slices"])] },
    { entry: "toast", components: [comp("Toaster", ["timeout"])] },
    { entry: "calendar", components: [comp("Calendar", ["month"])] },
    { entry: "dialog", components: [comp("Dialog", ["open"]), comp("Dialog.Title", ["children"])] },
    { entry: "orphans", components: [comp("Lonely", ["alone"])] },
  ],
};
function comp(name, props) {
  return { name, props: [...props, "className"].map((p) => ({ name: p })) };
}
const page = (slug, extra) => ({ slug, title: slug, description: "", examples: [], ...extra });

test("a page holds what it imports and its parts, not the rest of a shared entry", () => {
  const { index } = buildSearchIndex(
    [page("bar-chart", { entry: "charts", import: "BarChart" }), page("donut", { entry: "charts", import: "Donut" }), page("dialog", { title: "Dialog" })],
    { components: registry.components.filter((e) => e.entry !== "orphans") },
  );
  assert.deepEqual(index["bar-chart"].props, ["bars"]);
  assert.deepEqual(index["dialog"], { parts: ["Dialog.Title"], props: ["open", "children"], description: "", examples: [] });
});

test("an import from another entry and a component nobody imports still reach a page; others are reported", () => {
  const { index, missing } = buildSearchIndex([page("toast", { import: "toast" }), page("date-picker", { import: "DatePicker, Calendar" })], registry);
  assert.ok(index["toast"].parts.includes("Toaster") && index["toast"].props.includes("timeout"));
  assert.ok(index["date-picker"].parts.includes("Calendar"));
  assert.deepEqual(missing.sort(), ["charts: BarChart", "charts: Donut", "dialog: Dialog", "dialog: Dialog.Title", "orphans: Lonely"]);
});
