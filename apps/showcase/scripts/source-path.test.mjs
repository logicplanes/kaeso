import { test } from "node:test";
import assert from "node:assert/strict";
import { sourcePathOf, missingSources } from "./source-path.mjs";

const registry = {
  components: [
    {
      entry: "dialog",
      components: [
        { name: "Dialog", file: "src/components/dialog/dialog.tsx" },
        { name: "AlertDialog", file: "src/components/dialog/dialog.tsx" },
      ],
    },
    { entry: "kaeso", components: [{ name: "SourceBadge", file: "src/components/kaeso/source-badge.tsx" }] },
    { entry: "toast", components: [] },
  ],
};

test("a component that shares its entry's folder links to the file it is written in, not a folder named after its page", () => {
  assert.equal(
    sourcePathOf("alert-dialog", { title: "Alert dialog", entry: "dialog", import: "AlertDialog" }, registry),
    "packages/ui/src/components/dialog/dialog.tsx",
  );
  assert.equal(
    sourcePathOf("source-badge", { title: "Source badge", entry: "kaeso", import: "SourceBadge, SourceBadgeProps" }, registry),
    "packages/ui/src/components/kaeso/source-badge.tsx",
  );
});

test("a page's own source wins, and a page the registry doesn't know links to its entry's folder", () => {
  assert.equal(sourcePathOf("dialog", { title: "Dialog", source: "components/dialog" }, registry), "packages/ui/src/components/dialog");
  assert.equal(sourcePathOf("toast", { title: "Toast", import: "toast" }, registry), "packages/ui/src/components/toast");
});

test("pages whose source isn't in the checkout are named, so the build can stop instead of shipping a broken link", () => {
  const exists = (p) => p !== "packages/ui/src/components/gone";
  assert.deepEqual(missingSources({ ok: "packages/ui/src/components/dialog/dialog.tsx", gone: "packages/ui/src/components/gone" }, exists), [
    "gone: packages/ui/src/components/gone",
  ]);
});
