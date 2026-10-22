import { describe, expect, it } from "vitest";
import { ssoErrorMessage } from "./sso-errors";

const CODES = [
  "sso_off",
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
];

describe("single sign-on error codes", () => {
  it("has its own message for every code the gateway sends", () => {
    const generic = ssoErrorMessage("failed");
    const messages = CODES.map(ssoErrorMessage);
    expect(messages.every((m) => m !== generic)).toBe(true);
    expect(new Set(messages).size).toBe(CODES.length);
  });

  it("falls back to a general message for anything else", () => {
    expect(ssoErrorMessage("toString")).toBe("Single sign-on didn't work; try again.");
    expect(ssoErrorMessage("Your account is locked; call us")).toBe("Single sign-on didn't work; try again.");
  });
});
