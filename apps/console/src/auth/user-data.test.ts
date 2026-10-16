import { describe, expect, it } from "vitest";
import { forgetUserData, playgroundDraftKey } from "./user-data";

describe("forgetting a person's data", () => {
  it("removes what they did in this browser and keeps the device's settings", () => {
    const local = {
      "kaeso:recent": "[]",
      "kaeso:last": JSON.stringify({ org: "org_northwind" }),
      [playgroundDraftKey("proj_checkout")]: "Summarise order 4471",
      [playgroundDraftKey("proj_search")]: "Rank these results",
      "kaeso:theme": "dark",
      "kaeso:locale": "de",
      "kaeso:sidebar:width": "264",
      "kaeso:logs-columns": "[]",
    };
    for (const [key, value] of Object.entries(local)) window.localStorage.setItem(key, value);
    window.sessionStorage.setItem("kaeso:invite", "inv_token");
    window.sessionStorage.setItem("kaeso:return", "/orgs/org_northwind");

    forgetUserData();

    expect(Object.keys(window.localStorage).sort()).toEqual(["kaeso:locale", "kaeso:logs-columns", "kaeso:sidebar:width", "kaeso:theme"]);
    expect(window.sessionStorage.length).toBe(0);
  });
});
