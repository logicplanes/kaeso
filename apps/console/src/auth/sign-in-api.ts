import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import type { ConfirmSsoView, ConfirmWays as ConfirmWaysView, SignInMethodsView, SsoProviderView, TotpRequired } from "~/api/generated/types";

export type TotpChallenge = { challenge: string; expiresAt: string; attempts: number; codesLocked: boolean };
export type PasswordResult = { kind: "signed_in" } | ({ kind: "totp" } & TotpChallenge);

export function totpChallenge(required: TotpRequired): TotpChallenge {
  return { challenge: required.challenge, expiresAt: required.expires_at, attempts: required.attempts, codesLocked: required.codes_locked };
}

export async function signInWithPassword(email: string, password: string): Promise<PasswordResult> {
  const { data, response } = await api.POST("/platform/v1/auth/password", { body: { email, password } });
  if (response.status === 202 && data && "challenge" in data) return { kind: "totp", ...totpChallenge(data) };
  return { kind: "signed_in" };
}

export type CodeRefusal = "locked" | "ended" | "again";

export function codeRefusal(error: unknown, challenge: TotpChallenge, failures: number): CodeRefusal {
  if (!isApiError(error)) return "again";
  if (error.status === 403) return "locked";
  if (error.status === 401 && (failures >= challenge.attempts || Date.parse(challenge.expiresAt) <= Date.now())) return "ended";
  return "again";
}

export async function signInWithTotp(challenge: string, answer: { code: string } | { recoveryCode: string }): Promise<void> {
  await api.POST("/platform/v1/auth/totp", {
    body: "code" in answer ? { challenge, code: answer.code } : { challenge, recovery_code: answer.recoveryCode },
  });
}

export async function sendMagicLink(email: string): Promise<string> {
  return data(await api.POST("/platform/v1/auth/magic-link", { body: { email } })).message;
}

export async function startSso(body: ({ email: string } | { org: string }) & { next?: string }): Promise<string> {
  return data(await api.POST("/platform/v1/auth/oidc/start", { body })).authorization_url;
}

export function here(): string {
  return window.location.pathname + window.location.search;
}

export type SignInMethods = SignInMethodsView;
export type SsoProvider = SsoProviderView;

export const signInMethodsQuery = {
  queryKey: ["auth", "methods"] as const,
  queryFn: async (): Promise<SignInMethods> => data(await api.GET("/platform/v1/auth/methods")),
  staleTime: 5 * 60_000,
  retry: false,
  retryOnMount: false,
  networkMode: "always" as const,
};

export async function signInMethodsFor(email: string): Promise<SignInMethods> {
  return data(await api.POST("/platform/v1/auth/methods", { body: { email } }));
}

export type ConfirmWays = ConfirmWaysView;
export type ConfirmSso = ConfirmSsoView;

export function confirmWaysQuery(org: string | undefined) {
  return {
    queryKey: ["me", "sign-in-again", org ?? null] as const,
    queryFn: async (): Promise<ConfirmWays> => data(await api.GET("/platform/v1/me/sign-in-again", org ? { params: { query: { org } } } : {})),
    staleTime: 0,
    retry: false,
  };
}

export async function confirmWithSso(org: string, next: string): Promise<string> {
  return data(await api.POST("/platform/v1/me/sign-in-again/sso", { body: { org, next } })).authorization_url;
}

export async function emailPasswordLink(email: string): Promise<void> {
  await api.POST("/platform/v1/auth/password-reset", { body: { email } });
}
