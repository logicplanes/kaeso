import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { areaName, deliveryOptions, kindName, kindWords } from "./notification-kinds";

const catalogue = readFileSync(
  join(__dirname, "../../../../crates/kaeso-core/src/notifications/snapshots/kaeso_core__notifications__tests__notifications_reference.snap"),
  "utf8",
);
const rows = [...catalogue.matchAll(/^\| `([a-z_]+)` \| ([A-Za-z]+) \| ([^|]+) \|/gm)].map(([, kind = "", area = "", name = ""]) => ({
  kind,
  area: area.toLowerCase(),
  name: name.trim(),
}));

describe("notification words", () => {
  it("has its own words for every kind in the gateway's catalogue", () => {
    expect(rows).toHaveLength(23);
    for (const row of rows) {
      const words = kindWords({ kind: row.kind, name: "From the gateway", description: "From the gateway." });
      expect(words.name, row.kind).toBe(row.name);
      expect(words.description, row.kind).toMatch(/^\S.*\.$/);
      expect(kindName(row.kind, "From the gateway")).toBe(row.name);
    }
  });

  it("names every area of the catalogue", () => {
    for (const area of new Set(rows.map((r) => r.area))) expect(areaName({ area, name: "From the gateway" })).not.toBe("From the gateway");
  });

  it("falls back to the gateway's words for a kind or area it doesn't know", () => {
    expect(kindWords({ kind: "gossip", name: "Gossip", description: "Rumours." })).toEqual({ name: "Gossip", description: "Rumours." });
    expect(kindName("gossip", "Gossip")).toBe("Gossip");
    expect(areaName({ area: "billing", name: "Billing" })).toBe("Billing");
    expect(deliveryOptions().map((o) => o.value)).toEqual(["now", "daily", "off"]);
  });

  it("offers only the deliveries a kind takes, in the catalogue's order", () => {
    expect(deliveryOptions(["off", "now"]).map((o) => o.label)).toEqual(["Now", "Off"]);
    expect(deliveryOptions([])).toEqual([]);
  });
});
