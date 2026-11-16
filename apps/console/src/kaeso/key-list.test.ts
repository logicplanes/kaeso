import { describe, expect, it } from "vitest";
import { findKeys, isKeySort, keySortOptions } from "./key-list";

const k = (name: string, redacted: string, last_used_at: string | null, expires_at: string | null) => ({ name, redacted, last_used_at, expires_at });
const keys = [
  k("ci-deploy", "sk-kso-Ci1…aaaa", "2026-11-10T00:00:00Z", null),
  k("billing", "sk-kso-Bi2…bbbb", null, "2026-12-01T00:00:00Z"),
  k("ci-nightly", "sk-kso-Ci3…cccc", "2026-11-14T00:00:00Z", "2026-11-20T00:00:00Z"),
  k("archive-job", "sk-kso-Ar4…dddd", null, null),
];
const names = (list: { name: string }[]) => list.map((x) => x.name);

describe("findKeys", () => {
  it("finds keys by name or redacted secret, whatever the case", () => {
    expect(names(findKeys(keys, " CI- ", undefined))).toEqual(["ci-deploy", "ci-nightly"]);
    expect(names(findKeys(keys, "bbbb", undefined))).toEqual(["billing"]);
    expect(findKeys(keys, "nothing", undefined)).toEqual([]);
  });

  it("orders by name, newest use or soonest expiry, never used or expiring last", () => {
    expect(names(findKeys(keys, "", undefined))).toEqual(["archive-job", "billing", "ci-deploy", "ci-nightly"]);
    expect(names(findKeys(keys, "", "used"))).toEqual(["ci-nightly", "ci-deploy", "archive-job", "billing"]);
    expect(names(findKeys(keys, "", "expiry"))).toEqual(["ci-nightly", "billing", "archive-job", "ci-deploy"]);
  });

  it("knows its sorts", () => {
    expect([isKeySort("used"), isKeySort("expiry"), isKeySort("name"), isKeySort(3)]).toEqual([true, true, false, false]);
    expect(keySortOptions().map((o) => o.label)).toEqual(["Name", "Last used", "Expires soonest"]);
  });
});
