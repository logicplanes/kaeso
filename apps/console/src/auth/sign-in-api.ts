import { api, data } from "~/api/client";

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

export async function startSso(body: { email: string } | { org: string }): Promise<string> {
  return data(await api.POST("/platform/v1/auth/oidc/start", { body })).authorization_url;
}

export type SignInMethods = { password: boolean; magic_link: boolean; sso: boolean; sso_required: boolean; sign_up: boolean };

export const signInMethodsQuery = {
  queryKey: ["auth", "methods"] as const,
  queryFn: async (): Promise<SignInMethods> => data(await api.GET("/platform/v1/auth/methods")),
  staleTime: 5 * 60_000,
};

export async function signInMethodsFor(email: string): Promise<SignInMethods> {
  return data(await api.POST("/platform/v1/auth/methods", { body: { email } }));
}
