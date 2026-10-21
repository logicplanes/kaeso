import { describe, expect, it } from "vitest";
import { ApiError } from "~/api/errors";
import { codeRefusal, type TotpChallenge } from "./sign-in-api";

const challenge = (overrides: Partial<TotpChallenge> = {}): TotpChallenge => ({
  challenge: "chl_42",
  expiresAt: new Date(Date.now() + 5 * 60_000).toISOString(),
  attempts: 3,
  codesLocked: false,
  ...overrides,
});
const wrong = new ApiError({ type: "authentication", status: 401, message: "That code isn't valid or was already used." });

describe("a refused second-factor code", () => {
  it("leaves the challenge open while it has codes and time left", () => {
    expect(codeRefusal(wrong, challenge(), 2)).toBe("again");
  });

  it("ends the challenge after its last code or once it has expired", () => {
    expect(codeRefusal(wrong, challenge(), 3)).toBe("ended");
    expect(codeRefusal(wrong, challenge({ expiresAt: new Date(Date.now() - 1_000).toISOString() }), 1)).toBe("ended");
  });

  it("switches to a recovery code when the authenticator app is locked", () => {
    const locked = new ApiError({ type: "permission_denied", status: 403, message: "Too many wrong codes in a row." });
    expect(codeRefusal(locked, challenge(), 1)).toBe("locked");
  });

  it("lets the person try again after anything else", () => {
    expect(codeRefusal(new Error("offline"), challenge(), 3)).toBe("again");
    expect(codeRefusal(new ApiError({ type: "network", status: 0, message: "Can't reach the gateway." }), challenge(), 3)).toBe("again");
  });
});
