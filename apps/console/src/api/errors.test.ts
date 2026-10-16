import { describe, expect, it, vi } from "vitest";
import { apply } from "~/i18n";
import { errorFromResponse } from "./errors";

vi.mock("./gateway-errors", () => ({
  translateMessage: () => {
    throw new Error("The translations could not be loaded.");
  },
}));

describe("a refusal in another language", () => {
  it("keeps the gateway's own words when the translations cannot be loaded", async () => {
    apply({ locale: "de", messages: {}, ui: {}, provider: undefined });
    const response = new Response(JSON.stringify({ error: { type: "invalid_request", message: "Name is required." } }), { status: 400 });
    const error = await errorFromResponse(response);
    expect(error.message).toBe("Name is required.");
    expect(error.status).toBe(400);
  });
});
