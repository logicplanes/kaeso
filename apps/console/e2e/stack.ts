import { STACKS, resetTwoFactor, setupToken, signInLinkToken, urls, verifySsoDomain } from "../scripts/stack.mjs";

export const STACK = STACKS.e2e;
export const STACK_URLS = urls(STACK);
export const CONSOLE_URL = STACK.publicUrl;
export const CONSOLE_PORT = Number(new URL(CONSOLE_URL).port);
export const SSO_CONSOLE_URL = STACK.ssoPublicUrl;
export const SSO_CONSOLE_PORT = Number(new URL(SSO_CONSOLE_URL).port);

export function verifyDomain(domain: string, service = "gateway"): void {
  verifySsoDomain(STACK, domain, service);
}

export function resetUserTwoFactor(email: string): void {
  resetTwoFactor(STACK, email);
}

const tokens = new Map<string, string>();

export function setupLinkToken(service = "gateway"): string {
  const token = tokens.get(service) ?? setupToken(STACK, service);
  if (!token) throw new Error(`The e2e ${service} printed no setup link: start the suite on a fresh stack.`);
  tokens.set(service, token);
  return token;
}

export function operatorSignInToken(email: string, service = "gateway-sso"): string {
  const token = signInLinkToken(STACK, email, service);
  if (!token) throw new Error(`The e2e ${service} printed no sign-in link for ${email}.`);
  return token;
}
