import { describe, expect, it } from "vitest";
import { ssoErrorMessage } from "./sso-errors";

const CODES = [
  "no_sso",
  "invalid_link",
  "expired",
  "incomplete",
  "provider_refused",
  "provider_unreachable",
  "answer_unverified",
  "provider_incompatible",
  "email_unverified",
  "invalid_email",
  "domain_not_allowed",
  "no_verified_domain",
  "deactivated",
  "no_access",
  "accept_invite",
  "link_required",
  "linked_elsewhere",
  "signed_in_as_other",
  "org_scheduled",
  "throttled",
  "cannot_confirm",
];

describe("single sign-on error codes", () => {
  it("has its own message for every code the gateway sends", () => {
    const generic = ssoErrorMessage("failed");
    const messages = CODES.map((code) => ssoErrorMessage(code));
    expect(messages.every((m) => m !== generic)).toBe(true);
    expect(new Set(messages).size).toBe(CODES.length);
  });

  it("names the identity provider that can't confirm a recent sign-in when it's known", () => {
    expect(ssoErrorMessage("cannot_confirm", "Acme ID")).toBe("Acme ID can't confirm a recent sign-in; use your password or an email link.");
    expect(ssoErrorMessage("cannot_confirm")).toBe("The identity provider can't confirm a recent sign-in; use your password or an email link.");
  });

  it("falls back to a general message for anything else", () => {
    expect(ssoErrorMessage("toString")).toBe("Single sign-on didn't work; try again.");
    expect(ssoErrorMessage("Your account is locked; call us")).toBe("Single sign-on didn't work; try again.");
  });
});
