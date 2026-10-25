import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Badge, Button, DialogScope, SettingsGroup, SettingsRow, SettingsSection } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { SessionListItem } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { LoadFailed } from "~/features/org/load-failed";
import { sessionMethodLabel } from "~/format/labels";
import { describeUserAgent } from "~/format/user-agent";
import { SettingsLoading, SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";

const sessionsQuery = {
  queryKey: ["me", "sessions"],
  queryFn: async (): Promise<SessionListItem[]> => data(await api.GET("/platform/v1/me/sessions")).data,
};

export function SessionsPage() {
  const sessions = useQuery(sessionsQuery);
  const [ending, setEnding] = useState<SessionListItem | null>(null);
  const [endOpen, setEndOpen] = useState(false);
  const [all, setAll] = useState(false);
  const end = (s: SessionListItem) => {
    setEnding(s);
    setEndOpen(true);
  };
  const list = [...(sessions.data ?? [])].sort(
    (a, b) =>
      Number(b.current) - Number(a.current) ||
      Date.parse(b.last_used_at) - Date.parse(a.last_used_at) ||
      Date.parse(b.signed_in_at) - Date.parse(a.signed_in_at),
  );
  const others = list.filter((s) => !s.current).length;
  return (
    <SettingsPage
      title={t("account.sessions", "Sessions")}
      context={t("account.title", "Account")}
      description={t("account.sessionsHint", "Browsers signed in to your account.")}
      actions={
        <Button size="sm" disabled={!others} onClick={() => setAll(true)}>
          {t("account.signOutEverywhere", "Sign out everywhere")}
        </Button>
      }
    >
      {sessions.isError ? (
        <LoadFailed title={t("account.sessionsFailed", "Your sessions couldn't load")} />
      ) : sessions.isPending ? (
        <SettingsLoading rows={3} />
      ) : (
        <SettingsSection>
          <SettingsGroup>
            {list.map((s) => (
              <SettingsRow
                key={s.id}
                title={
                  <span className="flex flex-wrap items-center gap-8">
                    {describeUserAgent(s.user_agent)}
                    {s.current && <Badge tone="success">{t("account.thisDevice", "This device")}</Badge>}
                    <Badge>{sessionMethodLabel(s.method)}</Badge>
                    {s.second_factor && <Badge>{t("account.twoFactorBadge", "Two-factor")}</Badge>}
                  </span>
                }
                description={t("account.sessionDetail", "{address} · signed in {signedIn} · used {used}", {
                  address: s.ip ?? t("account.unknownAddress", "Unknown address"),
                  signedIn: formatRelative(s.signed_in_at),
                  used: formatRelative(s.last_used_at),
                })}
                control={
                  s.current ? undefined : (
                    <Button size="sm" variant="ghost" onClick={() => end(s)}>
                      {t("account.endSession", "End session")}
                    </Button>
                  )
                }
              />
            ))}
          </SettingsGroup>
        </SettingsSection>
      )}
      {ending && (
        <DialogScope open={endOpen}>
          <EndSessionDialog session={ending} open={endOpen} onOpenChange={setEndOpen} />
        </DialogScope>
      )}
      <DialogScope open={all}>
        <SignOutEverywhereDialog open={all} onOpenChange={setAll} />
      </DialogScope>
    </SettingsPage>
  );
}

function EndSessionDialog({ session, open, onOpenChange }: { session: SessionListItem; open: boolean; onOpenChange: (open: boolean) => void }) {
  const end = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/me/sessions/{session}", { params: { path: { session: session.id } } });
    },
    invalidates: [["me", "sessions"]],
    onSuccess: () => {
      onOpenChange(false);
      toast(t("account.sessionEnded", "Session ended"));
    },
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("account.endSessionConfirm", "End this session?")}
      description={t("account.endSessionConfirmHint", "{device} is signed out at once.", { device: describeUserAgent(session.user_agent) })}
      confirmLabel={t("account.endSession", "End session")}
      loading={end.isPending}
      onConfirm={() => end.mutateAsync()}
    />
  );
}

function SignOutEverywhereDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const revokeAll = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/me/sessions/revoke-all")),
    invalidates: [["me", "sessions"]],
    onSuccess: (r) => {
      onOpenChange(false);
      toast(t("account.signedOutOthers", "{count, plural, one {Signed out of # other session} other {Signed out of # other sessions}}", { count: r.revoked }));
    },
  });
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      tone="danger"
      title={t("account.signOutEverywhereConfirm", "Sign out everywhere else?")}
      description={t("account.signOutEverywhereConfirmHint", "Every other browser signed in to your account is signed out; this one stays signed in.")}
      confirmLabel={t("account.signOutEverywhere", "Sign out everywhere")}
      loading={revokeAll.isPending}
      onConfirm={() => revokeAll.mutateAsync()}
    />
  );
}
