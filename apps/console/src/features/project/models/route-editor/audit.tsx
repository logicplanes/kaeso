import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AppFrame, Button, icons } from "@logicplanes/kaeso-ui";
import type { AuditEventView, RouteStep } from "~/api/generated/types";
import type { AuditSearch } from "~/api/queries/audit";
import { projectMembersQuery } from "~/api/queries/members";
import { useProjectRights } from "~/auth/rights";
import { usePartyLabel } from "~/features/project/keys/party";
import { AuditLog } from "~/kaeso/audit-log";
import { t } from "~/i18n";
import { fromRoute, sameRoute, toRoute } from "./draft";
import { modelRouteOf } from "./route-json";

export type Version = { name: string; route: RouteStep };

export function versionOf(e: AuditEventView): Version | undefined {
  const after = e.after as { name?: unknown; route?: unknown; catalogue_model_id?: unknown } | null | undefined;
  if (!after || typeof after.name !== "string") return undefined;
  const route = (after.route as RouteStep | undefined) ?? (typeof after.catalogue_model_id === "string" ? modelRouteOf(after.catalogue_model_id) : undefined);
  return route ? { name: after.name, route } : undefined;
}

export function AliasAuditLog({
  org,
  project,
  alias,
  current,
  readOnly,
  onRestore,
}: {
  org: string;
  project: string;
  alias: string;
  current: Version;
  readOnly: boolean;
  onRestore: (v: Version) => void;
}) {
  const [search, setSearch] = useState<AuditSearch>({});
  const rights = useProjectRights(org, project);
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: rights.has("project.members.view") });
  const partyLabel = usePartyLabel(org, project);
  const restore = (e: AuditEventView) => {
    const v = versionOf(e);
    if (!v || readOnly) return null;
    const same = v.name === current.name.trim() && sameRoute(toRoute(fromRoute(v.route)), current.route);
    return (
      <Button
        variant="primary"
        leadingIcon={icons.undo}
        disabled={same}
        {...(same ? { disabledReason: t("models.route.alreadyShown", "This is the route in the editor.") } : {})}
        onClick={() => onRestore(v)}
      >
        {t("models.route.restore", "Restore this version")}
      </Button>
    );
  };
  return (
    <AppFrame.Body padding="none">
      <AuditLog
        scope={{ org, project }}
        search={search}
        onSearch={setSearch}
        partyLabel={partyLabel}
        actors={(members.data ?? []).map((m) => m.user)}
        about={alias}
        eventActions={restore}
      />
    </AppFrame.Body>
  );
}
