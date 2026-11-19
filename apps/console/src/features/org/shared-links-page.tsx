import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Badge, Button, DialogScope, Empty, List, Text } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api } from "~/api/client";
import type { SharedLinkView } from "~/api/generated/types";
import { orgProjectsQuery } from "~/api/queries/projects";
import { sharedLinksKey, sharedLinksQuery } from "~/api/queries/shared-links";
import { useApiMutation } from "~/auth/mutation";
import { useOrgRights, useSession } from "~/auth/session";
import { fullDayLabel } from "~/format/period";
import { t } from "~/i18n";
import { LastUsed } from "~/kaeso/last-used";
import { toast } from "~/shell/toast";
import { LoadFailed } from "./load-failed";
import { OrgPage } from "./org-page";
import { useOrgPartyName } from "./party";

export function kindLabel(kind: string): string {
  switch (kind) {
    case "member":
      return t("org.shared.kindMember", "Member stats");
    case "budget":
      return t("org.shared.kindBudget", "Budget status");
    case "project_card":
      return t("org.shared.kindCard", "Project summary");
    default:
      return t("org.shared.kindAnalytics", "Analytics");
  }
}

// Budgets and summaries show the present; usage links show a window of days, live or frozen.
function periodLabel(l: SharedLinkView): string | null {
  if (l.kind !== "analytics" && l.kind !== "member") return null;
  if (l.live) return l.days ? t("org.shared.lastDays", "{count, plural, one {Last day} other {Last # days}}", { count: l.days }) : null;
  return l.from && l.to ? t("org.shared.snapshot", "{from} to {to}", { from: fullDayLabel(l.from), to: fullDayLabel(l.to) }) : null;
}

export function SharedLinksPage({ org }: { org: string }) {
  const rights = useOrgRights(org);
  const session = useSession();
  const links = useQuery({ ...sharedLinksQuery(org), enabled: rights.has("usage.share") });
  const projects = useQuery(orgProjectsQuery(org));
  const party = useOrgPartyName(org);
  const [revoking, setRevoking] = useState<SharedLinkView | null>(null);
  const manages = rights.has("usage.share.manage");
  const items = (links.data ?? []).map((l) => {
    const project = l.project ? (projects.data?.find((p) => p.id === l.project)?.name ?? l.project) : undefined;
    const mine = l.created_by === session.user.id;
    return {
      id: l.id,
      textValue: l.title,
      title: (
        <span className="flex min-w-0 items-center gap-8">
          <span className="truncate">{l.title}</span>
          <Badge tone={l.audience === "anyone" ? "warning" : "outline"}>
            {l.audience === "anyone" ? t("org.shared.anyone", "Anyone with the link") : t("org.shared.members", "Members")}
          </Badge>
        </span>
      ),
      subtitle: [kindLabel(l.kind), periodLabel(l), project ?? t("org.shared.wholeOrg", "Whole organization"), party({ kind: "user", id: l.created_by })]
        .filter(Boolean)
        .join(" · "),
      meta: (
        <Text variant="small" tone="secondary">
          {t("org.shared.views", "{count, plural, one {# view} other {# views}}", { count: l.views })}
          {" · "}
          {l.expires_at ? t("org.shared.expires", "Expires {date}", { date: formatDate(l.expires_at) }) : t("org.shared.neverExpires", "Never expires")}
        </Text>
      ),
      date: <LastUsed at={l.last_opened_at ?? null} />,
      actions:
        manages || mine ? (
          <Button size="xs" variant="ghost" onClick={() => setRevoking(l)}>
            {t("org.shared.revoke", "Revoke")}
          </Button>
        ) : undefined,
    };
  });
  return (
    <OrgPage
      org={org}
      title={t("org.shared.title", "Shared links")}
      hidden={t("org.shared.hidden", "You can't see shared links here")}
      right="usage.share"
      description={
        manages
          ? t("org.shared.description", "Read-only links to the organization's usage that still work.")
          : t("org.shared.descriptionOwn", "Your read-only links to the organization's usage that still work.")
      }
    >
      <List
        aria-label={t("org.shared.title", "Shared links")}
        items={items}
        wrapSubtitle
        loading={links.isPending}
        empty={
          links.isError ? (
            <LoadFailed title={t("org.shared.loadFailed", "The shared links couldn't load")} size="md" />
          ) : (
            <Empty
              size="md"
              title={t("org.shared.none", "No shared links")}
              description={t("org.shared.noneHint", "Share Analytics, a member's stats or a project's budgets from their pages.")}
            />
          )
        }
      />
      <DialogScope open={!!revoking}>{revoking && <RevokeDialog org={org} link={revoking} onClose={() => setRevoking(null)} />}</DialogScope>
    </OrgPage>
  );
}

function RevokeDialog({ org, link, onClose }: { org: string; link: SharedLinkView; onClose: () => void }) {
  const revoke = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/orgs/{org}/shared-links/{link}", { params: { path: { org, link: link.id } } });
    },
    invalidates: [sharedLinksKey(org)],
    onSuccess: () => toast(t("org.shared.revoked", "{title} no longer works", { title: link.title })),
  });
  return (
    <AlertDialog
      open
      onOpenChange={(o) => !o && onClose()}
      tone="danger"
      title={t("org.shared.revokeTitle", "Revoke {title}?", { title: link.title })}
      description={t("org.shared.revokeHint", "Whoever has the link sees nothing from now on; opened {count, plural, one {# time} other {# times}}.", {
        count: link.views,
      })}
      confirmLabel={t("org.shared.revoke", "Revoke")}
      loading={revoke.isPending}
      onConfirm={() => revoke.mutateAsync()}
    />
  );
}
