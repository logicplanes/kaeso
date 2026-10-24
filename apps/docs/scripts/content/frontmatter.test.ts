import { describe, expect, it } from "vitest";
import { parseFrontmatter } from "./frontmatter.ts";

const page = (fm: string, body = "Text.") => `---\n${fm}\n---\n${body}`;

describe("parseFrontmatter", () => {
  it("reads the fields and the body", () => {
    const p = parseFrontmatter(page('title: Keys\ndescription: "Create a key: then use it."\norder: 2\nnav: All keys', "\nBody"));
    expect(p.problems).toEqual([]);
    expect(p.data).toEqual({ title: "Keys", description: "Create a key: then use it.", order: 2, nav: "All keys" });
    expect(p.body).toBe("\nBody");
  });

  it("accepts Windows line endings and a byte order mark", () => {
    const p = parseFrontmatter("﻿---\r\ntitle: Keys\r\ndescription: Keys for apps.\r\n---\r\nBody");
    expect(p.problems).toEqual([]);
    expect(p.body).toBe("Body");
  });

  it("names what is missing or wrong", () => {
    expect(parseFrontmatter("# Keys").problems[0]).toMatch(/starts without frontmatter/);
    const p = parseFrontmatter(page("titel: Keys\norder: first\nnav:"));
    expect(p.problems.join("\n")).toMatch(/unknown field `titel`/);
    expect(p.problems.join("\n")).toMatch(/no `title`/);
    expect(p.problems.join("\n")).toMatch(/no `description`/);
    expect(p.problems.join("\n")).toMatch(/`order` must be a whole number/);
    expect(p.problems.join("\n")).toMatch(/`nav` is empty/);
  });

  it("keeps descriptions to one short sentence", () => {
    expect(parseFrontmatter(page("title: A\ndescription: One sentence. And another.")).problems).toEqual([expect.stringMatching(/more than one sentence/)]);
    expect(parseFrontmatter(page(`title: A\ndescription: ${"word ".repeat(40)}`)).problems).toEqual([expect.stringMatching(/characters/)]);
    expect(parseFrontmatter(page("title: A\ndescription: Works with version 1.2.0 and later.")).problems).toEqual([]);
  });

  it("skips blank lines and comments, unquotes single and double quotes, and names a line that isn't a field or a field set twice", () => {
    const p = parseFrontmatter(page("# A comment\n\ntitle: 'It''s keys'\ndescription: \"Say \\\"hi\\\" to keys.\"\nnot a field\ntitle: Keys"));
    expect(p.data.description).toBe('Say "hi" to keys.');
    expect(p.problems).toEqual(["frontmatter line 6 is not `key: value`: not a field", "frontmatter sets `title` twice"]);
    expect(parseFrontmatter(page("title: 'It''s keys'\ndescription: Keys.")).data.title).toBe("It's keys");
    expect(parseFrontmatter("---\ntitle: Keys\ndescription: Keys.\n---").body).toBe("");
  });

  it("counts a Chinese description's sentences by their full stops", () => {
    expect(parseFrontmatter(page("title: A\ndescription: 创建密钥。然后使用它。")).problems).toEqual([expect.stringMatching(/more than one sentence/)]);
    expect(parseFrontmatter(page("title: A\ndescription: 创建密钥并使用它。")).problems).toEqual([]);
  });
});
