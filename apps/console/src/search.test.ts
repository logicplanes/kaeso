import { describe, expect, it } from "vitest";
import { patchSearch } from "./search";

type Search = { q?: string; page?: number; stream?: boolean; open?: boolean };

describe("patchSearch", () => {
  it("merges the patch and leaves out what is unset or empty", () => {
    expect(patchSearch<Search>({ q: "ada", page: 2 }, { q: "", stream: false })).toEqual({ page: 2, stream: false });
    expect(patchSearch<Search>({ page: 0 }, { q: undefined })).toEqual({ page: 0 });
  });

  it("drops the named one-time keys", () => {
    expect(patchSearch<Search>({ q: "ada", open: true }, {}, ["open"])).toEqual({ q: "ada" });
  });
});
