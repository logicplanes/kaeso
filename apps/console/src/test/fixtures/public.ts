import type { AcceptedView, EmailSent, InviteInfo, MemberUser, SignInMethodsView, SignedIn, StartResponse, TotpRequired } from "~/api/generated/types";
import { ORG, user, whoAmI } from "../fixtures";

const inFiveMinutes = () => new Date(Date.now() + 5 * 60_000).toISOString();
const inThreeDays = () => new Date(Date.now() + 3 * 86_400_000).toISOString();

export function methods(overrides: Partial<SignInMethodsView> = {}): SignInMethodsView {
  return { password: true, magic_link: true, sso: false, sso_required: false, sign_up: false, set_up: false, ...overrides } as SignInMethodsView;
}

export function signedInReply(): SignedIn {
  return { status: "signed_in", user: user(), session: whoAmI().session };
}

export function totpRequired(overrides: Partial<TotpRequired> = {}): TotpRequired {
  return { status: "totp_required", challenge: "chl_42", expires_at: inFiveMinutes(), ...overrides };
}

export function emailSent(message: string): EmailSent {
  return { status: "sent", message };
}

export function ssoStart(url = "https://idp.example.com/authorize?state=s1"): StartResponse {
  return { authorization_url: url };
}

export function invite(overrides: Partial<InviteInfo> = {}): InviteInfo {
  return {
    email: "grace@example.com",
    account_exists: false,
    expires_at: inThreeDays(),
    org: { id: ORG, name: "Northwind Labs" },
    role: { id: "developer", kind: "builtin", name: "Developer" },
    ...overrides,
  };
}

export function accepted(org: string | null = ORG): AcceptedView {
  return { status: "accepted", org_id: org };
}

export function memberUser(name: string, id = `usr_${name.toLowerCase().split(" ")[0] ?? "x"}`): MemberUser {
  return { id, name, email: `${id.slice(4)}@example.com`, state: "active", avatar_url: null };
}
