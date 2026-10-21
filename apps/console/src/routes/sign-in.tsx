import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { createFileRoute, redirect } from "@tanstack/react-router";
import { SignInPage } from "~/features/sign-in/sign-in-page";
import { takeFragment } from "~/auth/fragment";
import { internalPath, safeNext } from "~/auth/next";
import { sessionQuery } from "~/auth/session-queries";
import { signInPageQuery, type SignInMethods } from "~/auth/sign-in-api";
import { t } from "~/i18n";

const NOTICES: Record<string, () => string> = {
  "password-changed": () => t("auth.notice.passwordChanged", "Password changed; sign in with the new one"),
  "signed-out": () => t("auth.notice.signedOut", "You're signed out"),
};

const ORG_ID = /^org_[A-Za-z0-9]{1,64}$/;

const text = (search: Record<string, unknown>, key: string) => (typeof search[key] === "string" ? { [key]: search[key] } : {});

const signInSearch = (search: Record<string, unknown>): { next?: string; org?: string; done?: string; start?: string } => ({
  ...(typeof search.next === "string" && internalPath(search.next) ? { next: search.next } : {}),
  ...text(search, "org"),
  ...text(search, "done"),
  ...(search.start === "sso" ? { start: "sso" } : {}),
});

export const Route = createFileRoute("/sign-in")({
  validateSearch: signInSearch,
  beforeLoad: async ({ context, search }): Promise<{ signedInAs: string | null }> => {
    const current = await context.queryClient.fetchQuery({ ...sessionQuery, staleTime: 0, retry: false }).then(
      (who) => who,
      () => null,
    );
    if (!current) return { signedInAs: null };
    const failed = new URLSearchParams(window.location.hash.slice(1)).get("sso_error");
    if (!failed && search.org && ORG_ID.test(search.org)) return { signedInAs: current.user.email ?? current.user.name };
    const next = safeNext(search.next);
    throw redirect({ href: failed ? `${next}#sso_error=${encodeURIComponent(failed)}` : next, replace: true });
  },
  component: SignIn,
});

function SignIn() {
  const search = Route.useSearch({ select: signInSearch, structuralSharing: true });
  const org = search.org && ORG_ID.test(search.org) ? search.org : undefined;
  const done = search.done && Object.hasOwn(NOTICES, search.done) ? search.done : undefined;
  const [ssoError] = useState(() => takeFragment("sso_error"));
  const [quiet] = useState(() => ssoError !== null || done === "signed-out");
  const queryClient = useQueryClient();
  const signedInAs = Route.useRouteContext({ select: (c) => c.signedInAs });
  const [page, setPage] = useState<SignInMethods | null | undefined>(() => queryClient.getQueryData(signInPageQuery(org).queryKey));
  useEffect(() => {
    let current = true;
    queryClient.fetchQuery(signInPageQuery(org)).then(
      (methods) => current && setPage(methods),
      () => current && setPage(null),
    );
    return () => {
      current = false;
    };
  }, [org, queryClient]);
  return (
    <SignInPage
      next={search.next}
      org={org}
      page={page}
      ssoError={ssoError}
      notice={done ? NOTICES[done]?.() : undefined}
      quiet={quiet}
      startNow={search.start === "sso"}
      signedInAs={signedInAs}
    />
  );
}
