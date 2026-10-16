import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Button, Input, SensitiveInput, Spinner, Text } from "@logicplanes/kaeso-ui";
import { PublicPage } from "~/shell/public-page";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { isApiError } from "~/api/errors";
import { toast } from "~/shell/toast";
import type { InviteInfo } from "~/api/generated/types";
import { takeFragment } from "~/auth/fragment";
import { sessionQuery } from "~/auth/session";
import { usePageTitle } from "~/shell/title";
import { fieldError } from "~/api/field-error";
import { passwordHint, passwordOk, useSubmit } from "./form";
import { t } from "~/i18n";
import { readStored, writeStored } from "~/storage";

const PENDING = "kaeso:invite";

function inviteToken(): string | null {
  const fromLink = takeFragment("token");
  if (fromLink) writeStored(PENDING, fromLink, "session");
  return fromLink ?? readStored(PENDING, "session");
}

function forget() {
  writeStored(PENDING, null, "session");
}

export function InvitePage() {
  usePageTitle(t("public.invitation", "Invitation"));
  const [token] = useState(inviteToken);
  if (!token) {
    return (
      <PublicPage
        title={t("public.needsInvitationLink", "This page needs the invitation link")}
        description={t("public.needsInvitationLinkHint", "Open the link from the invitation email; it ends with #token=…")}
      >
        <Button size="xl" fullWidth href="/sign-in">
          {t("public.backToSignIn", "Back to sign in")}
        </Button>
      </PublicPage>
    );
  }
  return <Invitation token={token} />;
}

function Invitation({ token }: { token: string }) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const lookup = useQuery({
    queryKey: ["invite-lookup", token],
    retry: false,
    queryFn: async (): Promise<InviteInfo> => data(await api.POST("/platform/v1/auth/invites/lookup", { body: { token } })),
  });
  const session = useQuery({ ...sessionQuery, retry: false });
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [accepted, setAccepted] = useState<string | null | undefined>(undefined);
  const [byLink, setByLink] = useState(false);
  const { busy, error, submit } = useSubmit();
  const unusable = lookup.isError && isApiError(lookup.error) && (lookup.error.type === "not_found" || lookup.error.type === "invalid_request");
  const [signingOut, setSigningOut] = useState(false);
  const signOut = async () => {
    setSigningOut(true);
    try {
      await api.POST("/platform/v1/auth/logout");
      queryClient.removeQueries({ queryKey: ["session"] });
      await session.refetch();
    } catch (e) {
      toast.error(isApiError(e) ? e.message : t("common.unreachable", "Can't reach the gateway. Try again."));
    } finally {
      setSigningOut(false);
    }
  };

  useEffect(() => {
    if (unusable) forget();
  }, [unusable]);

  useEffect(() => {
    if (accepted === undefined) return;
    forget();
    void queryClient.resetQueries().then(() => navigate({ href: accepted ? `/orgs/${accepted}` : "/", replace: true }));
  }, [accepted, navigate, queryClient]);

  const accept = (body: { name?: string; password?: string }) => {
    setByLink(body.password === undefined);
    return submit(async () => {
      const result = data(await api.POST("/platform/v1/auth/invites/accept", { body: { token, ...body } }));
      setAccepted(result.org_id ?? null);
    });
  };

  if (accepted !== undefined) {
    return (
      <PublicPage title={t("public.invitationAccepted", "Invitation accepted")}>
        <div className="flex justify-center show-after-delay">
          <Spinner label={t("public.openingOrg", "Opening the organization")} />
        </div>
      </PublicPage>
    );
  }
  if (lookup.isPending || session.isPending) {
    return (
      <PublicPage title={t("public.openingInvitation", "Opening the invitation…")}>
        <div className="flex justify-center show-after-delay">
          <Spinner label={t("public.lookingUpInvitation", "Looking up the invitation")} />
        </div>
      </PublicPage>
    );
  }
  if (lookup.isError && !unusable) {
    return (
      <PublicPage
        title={t("public.invitationNotOpened", "The invitation couldn't be opened")}
        description={isApiError(lookup.error) ? lookup.error.message : t("common.unreachable", "Can't reach the gateway. Try again.")}
      >
        <Button variant="primary" size="xl" fullWidth loading={lookup.isFetching} onClick={() => void lookup.refetch()}>
          {t("common.tryAgain", "Try again")}
        </Button>
      </PublicPage>
    );
  }
  if (lookup.isError) {
    return (
      <PublicPage
        title={t("public.invitationUnusable", "This invitation can't be used")}
        description={t("public.invitationUnusableHint", "It expired, was revoked or was used; ask whoever invited you for a new one.")}
      >
        <Button size="xl" fullWidth href="/sign-in">
          {t("public.backToSignIn", "Back to sign in")}
        </Button>
      </PublicPage>
    );
  }

  const invite = lookup.data;
  const where = {
    target: invite.org ? (invite.role ? "role" : "org") : "install",
    org: invite.org?.name ?? "",
    role: invite.role?.name ?? "",
  };
  const title = invite.org ? t("public.joinOrg", "Join {org}", { org: invite.org.name }) : t("public.joinKaeso", "Join Kaeso");
  const expiry = { email: invite.email, when: formatRelative(invite.expires_at) };
  const expires = t("public.invitationExpires", "The invitation for {email} expires {when}.", expiry);
  const me = session.data?.user;

  if (me) {
    const mine = me.email?.toLowerCase() === invite.email.toLowerCase();
    return (
      <PublicPage
        title={title}
        description={
          mine
            ? t(
                "public.acceptToJoin",
                "{target, select, role {Accept to join {org} as {role}.} org {Accept to join {org}.} other {Accept to join this Kaeso install.}}",
                where,
              )
            : t("public.invitationForOther", "This invitation is for {email}; you're signed in as {me}.", { email: invite.email, me: me.email ?? me.name })
        }
      >
        <div className="flex flex-col gap-16">
          {mine ? (
            <Button variant="primary" size="xl" fullWidth loading={busy} onClick={() => void accept({})}>
              {t("public.acceptInvitation", "Accept invitation")}
            </Button>
          ) : (
            <Button size="xl" fullWidth loading={signingOut} onClick={() => void signOut()}>
              {t("public.signOutToAccept", "Sign out to accept")}
            </Button>
          )}
          <Text variant="small" tone="secondary" className="text-center">
            {expires}
          </Text>
        </div>
      </PublicPage>
    );
  }

  if (invite.account_exists) {
    return (
      <PublicPage
        title={title}
        description={t(
          "public.accountExists",
          "{target, select, role {An account with {email} exists. Sign in to accept and join {org} as {role}.} org {An account with {email} exists. Sign in to accept and join {org}.} other {An account with {email} exists. Sign in to accept and join this Kaeso install.}}",
          { ...where, email: invite.email },
        )}
      >
        <div className="flex flex-col gap-16">
          <Button variant="primary" size="xl" fullWidth onClick={() => void navigate({ to: "/sign-in", search: { next: "/invite" } })}>
            {t("public.signInToAccept", "Sign in to accept")}
          </Button>
          <Text variant="small" tone="secondary" className="text-center">
            {expires}
          </Text>
        </div>
      </PublicPage>
    );
  }

  const ready = name.trim() && passwordOk(password);
  return (
    <PublicPage
      title={title}
      description={t(
        "public.createToJoin",
        "{target, select, role {Create your account to join {org} as {role}.} org {Create your account to join {org}.} other {Create your account to join this Kaeso install.}}",
        where,
      )}
    >
      <form className="flex flex-col gap-16" onSubmit={(e) => (e.preventDefault(), ready && !busy && void accept({ name: name.trim(), password }))}>
        <Input label={t("common.email", "Email")} size="xl" value={invite.email} readOnly autoComplete="username" />
        <Input
          label={t("common.name", "Name")}
          size="xl"
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoComplete="name"
          autoFocus
          maxLength={100}
          error={fieldError(error, "name")}
        />
        <SensitiveInput
          label={t("public.password", "Password")}
          kind="password"
          size="xl"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete="new-password"
          description={passwordHint(password)}
          error={fieldError(error, "password")}
        />
        <Button type="submit" variant="primary" size="xl" fullWidth loading={busy && !byLink} disabled={!ready || busy}>
          {t("public.createAndJoin", "Create account and join")}
        </Button>
        <Button size="xl" fullWidth loading={busy && byLink} disabled={!name.trim() || busy} onClick={() => void accept({ name: name.trim() })}>
          {t("public.linkInstead", "Continue with a sign-in link instead")}
        </Button>
        <Text variant="small" tone="secondary" className="text-center">
          {t("public.withoutPassword", "Without a password, you sign in with a link by email. The invitation for {email} expires {when}.", expiry)}
        </Text>
      </form>
    </PublicPage>
  );
}
