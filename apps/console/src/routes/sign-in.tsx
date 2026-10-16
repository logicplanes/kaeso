import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { SignInPage } from "~/features/sign-in/sign-in-page";
import { takeFragment } from "~/auth/fragment";
import { internalPath } from "~/auth/next";
import { t } from "~/i18n";

const NOTICES = { "password-changed": () => t("auth.notice.passwordChanged", "Password changed; sign in with the new one") } as const;

const signInSearch = (search: Record<string, unknown>): { next?: string; done?: keyof typeof NOTICES } => ({
  ...(typeof search.next === "string" && internalPath(search.next) ? { next: search.next } : {}),
  ...(search.done === "password-changed" ? { done: "password-changed" as const } : {}),
});

export const Route = createFileRoute("/sign-in")({ validateSearch: signInSearch, component: SignIn });

function SignIn() {
  const { next, done } = Route.useSearch({ select: signInSearch, structuralSharing: true });
  const [ssoError] = useState(() => takeFragment("sso_error"));
  const notice = done && Object.hasOwn(NOTICES, done) ? NOTICES[done]() : undefined;
  return <SignInPage next={next} ssoError={ssoError} notice={notice} />;
}
