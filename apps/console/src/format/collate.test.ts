import { afterEach, describe, expect, it } from "vitest";
import { apply, load } from "~/i18n";
import { compareNames } from "./collate";

afterEach(async () => apply(await load("en")));

describe("sorting names", () => {
  it("sorts the way the language reads, numbers by value and case aside", async () => {
    apply(await load("de"));
    expect(["Zoe", "Ärzte", "Agent 10", "agent 9", "Ada"].sort(compareNames)).toEqual(["Ada", "agent 9", "Agent 10", "Ärzte", "Zoe"]);
  });

  it("follows the console's language", async () => {
    apply(await load("zh-CN"));
    expect(["张伟", "李娜", "王芳"].sort(compareNames)).toEqual(["李娜", "王芳", "张伟"]);
  });
});
