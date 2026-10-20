import { api, data } from "~/api/client";
import type { SignInMethodsView, SsoProviderView } from "~/api/generated/types";

export type PasswordResult = { kind: "signed_in" } | { kind: "totp"; challenge: string; expiresAt: string };

export async function signInWithPassword(email: string, password: string): Promise<PasswordResult> {
  const { data, response } = await api.POST("/platform/v1/auth/password", { body: { email, password } });
  if (response.status === 202 && data && "challenge" in data) return { kind: "totp", challenge: data.challenge, expiresAt: data.expires_at };
  return { kind: "signed_in" };
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

export const signInMethodsQuery = signInPageQuery();

export function signInPageQuery(org?: string) {
  return {
    queryKey: ["auth", "methods", org ?? null] as const,
    queryFn: async (): Promise<SignInMethods> => data(await api.GET("/platform/v1/auth/methods", org ? { params: { query: { org } } } : {})),
    staleTime: 5 * 60_000,
    retry: false,
    retryOnMount: false,
    networkMode: "always" as const,
  };
}

export async function signInMethodsFor(email: string): Promise<SignInMethods> {
  return data(await api.POST("/platform/v1/auth/methods", { body: { email } }));
}
