import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AppFrame, Banner, Button, Empty, PageHeader } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { toastApiError } from "~/api/toast-error";
import { t } from "~/i18n";
import { orgOwnersQuery, orgViewKey } from "~/api/queries/orgs";
import { useApiMutation } from "./mutation";
import { eitherOf } from "./people";
import { membershipsQuery, PROJECT_RIGHTS_KEY, sessionQuery, useMembership, useOrgRights } from "./session";
import { requestRecentSignIn } from "./recent-sign-in-store";
import { here, startSso } from "./sign-in-api";

export function NeedsTwoFactor({ orgName }: { orgName: string }) {
  return (
    <>
      <PageHeader start={<AppFrame.MenuButton />} title={orgName} />
      <AppFrame.Body>
        <Empty
          size="lg"
          title={t("auth.twoFactor.setUp", "Set up two-factor authentication")}
          description={t("auth.twoFactor.required", "{org} requires two-factor authentication. Set it up for your account, then sign in again with a code.", {
            org: orgName,
          })}
          actions={
            <Button variant="primary" href="/settings/account/security">
              {t("auth.twoFactor.setUp", "Set up two-factor authentication")}
            </Button>
          }
        />
      </AppFrame.Body>
    </>
  );
}

export function NeedsSso({ org, orgName }: { org: string; orgName: string }) {
  const start = useMutation({
    mutationFn: () => startSso({ org, next: here() }),
    onSuccess: (url) => window.location.assign(url),
    onError: (error) => toastApiError(error),
  });
  return (
    <>
      <PageHeader start={<AppFrame.MenuButton />} title={orgName} />
      <AppFrame.Body>
        <Empty
          size="lg"
          title={t("auth.sso.title", "Sign in through {org}'s identity provider", { org: orgName })}
          description={t("auth.sso.required", "{org} requires single sign-on. Your session works for your other organizations.", { org: orgName })}
          actions={
            <Button variant="primary" loading={start.isPending} onClick={() => start.mutate()}>
              {t("auth.continueWithSso", "Continue with SSO")}
            </Button>
          }
        />
      </AppFrame.Body>
    </>
  );
}

export function NeedsOtherSignIn({ orgName }: { orgName: string }) {
  const queryClient = useQueryClient();
  const signIn = async () => {
    if (await requestRecentSignIn("without-sso")) await queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey });
  };
  return (
    <>
      <PageHeader start={<AppFrame.MenuButton />} title={orgName} />
      <AppFrame.Body>
        <Empty
          size="lg"
          title={t("auth.otherSignIn.title", "Sign in another way to open {org}", { org: orgName })}
          description={t("auth.otherSignIn.description", "Your SSO sign-in opens only its own organization; sign in with your password or an email link.")}
          actions={
            <Button variant="primary" onClick={() => void signIn()}>
              {t("auth.otherSignIn.action", "Sign in another way")}
            </Button>
          }
        />
      </AppFrame.Body>
    </>
  );
}

function owners(names: string[]): string | null {
  const [first, second] = names;
  if (names.length === 1 && first !== undefined) return first;
  if (names.length === 2 && first !== undefined && second !== undefined) return eitherOf(first, second);
  return null;
}

function deletionSentence(canRestore: boolean, layout: "full" | "inline", day: string | null, who: string | null): string {
  if (canRestore && layout === "full")
    return day
      ? t("auth.deletion.fullOn", "It's read-only and its keys stopped working; on {day} it's deleted for good.", { day })
      : t("auth.deletion.fullSoon", "It's read-only and its keys stopped working; it's deleted for good soon.");
  if (canRestore)
    return day
      ? t("auth.deletion.inlineOn", "It's read-only until it's deleted for good on {day}.", { day })
      : t("auth.deletion.inline", "It's read-only until it's deleted for good.");
  if (who)
    return day
      ? t("auth.deletion.askPersonOn", "It's read-only until {day}; ask {person} to restore it.", { day, person: who })
      : t("auth.deletion.askPerson", "It's read-only until it's deleted; ask {person} to restore it.", { person: who });
  return day
    ? t("auth.deletion.askOwnerOn", "It's read-only until {day}; ask an owner to restore it.", { day })
    : t("auth.deletion.askOwner", "It's read-only until it's deleted; ask an owner to restore it.");
}

export function ScheduledForDeletion({ org, layout = "full" }: { org: string; layout?: "full" | "inline" }) {
  const queryClient = useQueryClient();
  const membership = useMembership(org);
  const rights = useOrgRights(org);
  const canRestore = rights.has("org.delete");
  const ownerList = useQuery({ ...orgOwnersQuery(org), enabled: !canRestore && rights.has("members.view") });
  const restore = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/restore", { params: { path: { org } } })),
    invalidates: [sessionQuery.queryKey, membershipsQuery.queryKey, orgViewKey(org), PROJECT_RIGHTS_KEY],
    onSuccess: (o) => toast.success(t("auth.deletion.restored", "{org} restored", { org: o.name })),
  });
  const due = membership?.org_deletion_due_at ?? null;
  useEffect(() => {
    if (!due) return;
    const refresh = () => {
      void queryClient.invalidateQueries({ queryKey: sessionQuery.queryKey });
      void queryClient.invalidateQueries({ queryKey: membershipsQuery.queryKey });
    };
    let every: ReturnType<typeof setInterval> | undefined;
    const wait = Math.max(0, new Date(due).getTime() - Date.now());
    const at =
      wait < 2 ** 31 - 1
        ? setTimeout(() => {
            refresh();
            every = setInterval(refresh, 60_000);
          }, wait)
        : undefined;
    return () => {
      clearTimeout(at);
      clearInterval(every);
    };
  }, [due, queryClient]);
  const day = due ? formatDate(due) : null;
  const who = owners((ownerList.data ?? []).map((m) => m.user.name));
  const sentence = deletionSentence(canRestore, layout, day, who);
  return (
    <Banner
      tone="danger"
      layout={layout}
      title={t("auth.deletion.title", "This organization is scheduled for deletion")}
      actions={
        canRestore ? (
          <Button size="sm" loading={restore.isPending} onClick={() => restore.mutate()}>
            {t("auth.deletion.restore", "Restore organization")}
          </Button>
        ) : undefined
      }
    >
      {sentence}
    </Banner>
  );
}
