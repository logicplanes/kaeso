import { STACKS, resetTwoFactor, setupToken, urls } from "../scripts/stack.mjs";

export const STACK = STACKS.e2e;
export const STACK_URLS = urls(STACK);
export const CONSOLE_URL = STACK.publicUrl;
export const CONSOLE_PORT = Number(new URL(CONSOLE_URL).port);

export function resetUserTwoFactor(email: string): void {
  resetTwoFactor(STACK, email);
}

export function setupLinkToken(): string {
  const token = setupToken(STACK);
  if (!token) throw new Error("The e2e gateway printed no setup link: start the suite on a fresh stack.");
  return token;
}
