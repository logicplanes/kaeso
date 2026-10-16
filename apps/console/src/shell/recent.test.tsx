import { describe, expect, it } from "vitest";
import { renderUi } from "~/test/render";
import { readRecent, recentHint, rememberRecent, useRememberRecent, type RecentEntity } from "./recent";

const entity = (id: string, kind: RecentEntity["kind"] = "key"): RecentEntity => ({ kind, id, label: `Item ${id}`, href: `/items/${id}` });

describe("recently opened items", () => {
  it("puts the latest first, keeps each item once and remembers eight", () => {
    for (let i = 1; i <= 9; i++) rememberRecent(entity(String(i)));
    rememberRecent(entity("5"));
    rememberRecent(entity("5", "model"));
    expect(readRecent().map((e) => `${e.kind}:${e.id}`)).toEqual(["model:5", "key:5", "key:9", "key:8", "key:7", "key:6", "key:4", "key:3"]);
  });

  it("ignores what isn't a list of items", () => {
    window.localStorage.setItem("kaeso:recent", JSON.stringify({ kind: "key" }));
    expect(readRecent()).toEqual([]);
    window.localStorage.setItem("kaeso:recent", "not json");
    expect(readRecent()).toEqual([]);
    window.localStorage.setItem("kaeso:recent", JSON.stringify([entity("1"), { kind: "key", id: "2", label: 2, href: "/x" }, 0]));
    expect(readRecent()).toEqual([entity("1")]);
  });

  it("remembers what a page shows once it has loaded", async () => {
    function Opened({ entity: shown }: { entity: RecentEntity | null }) {
      useRememberRecent(shown);
      return null;
    }
    await renderUi(
      <>
        <Opened entity={null} />
        <Opened entity={entity("7", "member")} />
      </>,
    );
    expect(readRecent()).toEqual([entity("7", "member")]);
  });

  it("names each kind", () => {
    expect((["key", "model", "member", "service-account", "request"] as const).map(recentHint)).toEqual([
      "Key",
      "Model",
      "Member",
      "Service account",
      "Request",
    ]);
  });
});
