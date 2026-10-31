import { useQuery } from "@tanstack/react-query";
import { Button, Select, SettingsGroup, SettingsRow, SettingsSection, Switch } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { ProjectResponseCache } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

const projectResponseCacheQuery = (org: string, project: string) => ({
  queryKey: ["orgs", org, "projects", project, "response-cache"] as const,
  queryFn: async (): Promise<ProjectResponseCache> =>
    data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/response-cache", { params: { path: { org, project } } })),
});

const TTLS = [300, 900, 3600, 21600, 86400, 604800] as const;

function ttlLabel(seconds: number): string {
  if (seconds % 86400 === 0) return t("project.responseCache.days", "{count, plural, one {# day} other {# days}}", { count: seconds / 86400 });
  if (seconds % 3600 === 0) return t("project.responseCache.hours", "{count, plural, one {# hour} other {# hours}}", { count: seconds / 3600 });
  if (seconds % 60 === 0) return t("project.responseCache.minutes", "{count, plural, one {# minute} other {# minutes}}", { count: seconds / 60 });
  return t("project.responseCache.seconds", "{count, plural, one {# second} other {# seconds}}", { count: seconds });
}

export function ResponseCacheSection({ org, project, canChange, archived }: { org: string; project: string; canChange: boolean; archived: boolean }) {
  const needs = useNeedsRight(org, project);
  const query = projectResponseCacheQuery(org, project);
  const settings = useQuery(query);
  const path = { params: { path: { org, project } } };
  const save = useApiMutation({
    mutationFn: async (body: { enabled?: boolean; ttl_seconds?: number }) =>
      data(await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/response-cache", { ...path, body })),
    invalidates: [query.queryKey],
    onSuccess: (s, body) =>
      toast.success(
        body.enabled === undefined
          ? t("project.responseCache.ttlSaved", "Replies kept for {ttl}", { ttl: ttlLabel(s.ttl_seconds) })
          : s.enabled
            ? t("project.responseCache.on", "Response cache turned on")
            : t("project.responseCache.off", "Response cache turned off"),
      ),
  });
  const clear = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/response-cache/clear", path)),
    invalidates: [query.queryKey],
    onSuccess: () => toast.success(t("project.responseCache.cleared", "Response cache cleared")),
  });
  const s = settings.data;
  const blocked = !!s && !s.enabled && (!s.available || !s.org_allowed);
  const locked = !canChange
    ? needs("project.update")
    : archived
      ? t("project.settings.archivedReadOnly", "An archived project can't change")
      : blocked
        ? !s.available
          ? t("project.responseCache.unavailable", "Off on this install")
          : t("project.responseCache.orgOff", "Off for the organization")
        : undefined;
  const ttls = s && !TTLS.some((v) => v === s.ttl_seconds) ? [...TTLS, s.ttl_seconds].sort((a, b) => a - b) : [...TTLS];
  return (
    <SettingsSection
      title={t("project.responseCache.title", "Response cache")}
      description={t("project.responseCache.hint", "Answers a repeated identical request from a stored reply, at no provider cost.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("project.responseCache.use", "Cache replies")}
          description={
            !s
              ? undefined
              : !s.available
                ? t("project.responseCache.unavailableHint", "This install doesn't offer the response cache.")
                : !s.org_allowed
                  ? t("project.responseCache.orgOffHint", "Off for the organization; it can't be turned on here.")
                  : s.enabled
                    ? t("project.responseCache.onHint", "Non-streamed requests sent twice get the stored reply.")
                    : t("project.responseCache.offHint", "Every request goes to the provider.")
          }
          control={
            <Switch
              aria-label={t("project.responseCache.use", "Cache replies")}
              checked={!!s?.effective}
              disabled={!s || !!locked}
              {...(locked ? { disabledReason: locked } : {})}
              loading={save.isPending && save.variables?.enabled !== undefined}
              onCheckedChange={(on) => save.mutate({ enabled: on })}
            />
          }
        />
        {s?.effective && (
          <>
            <SettingsRow
              title={t("project.responseCache.ttl", "Keep replies for")}
              description={t("project.responseCache.ttlHint", "Applies to replies stored from now on.")}
              control={
                <div className="w-160">
                  <Select
                    aria-label={t("project.responseCache.ttl", "Keep replies for")}
                    size="sm"
                    fullWidth
                    value={String(s.ttl_seconds)}
                    disabled={!!locked}
                    onValueChange={(v) => save.mutate({ ttl_seconds: Number(v) })}
                    options={ttls.map((v) => ({ value: String(v), label: ttlLabel(v) }))}
                  />
                </div>
              }
            />
            <SettingsRow
              title={t("project.responseCache.clear", "Clear cache")}
              description={t("project.responseCache.clearHint", "Stored replies stop being served at once.")}
              control={
                <Button
                  variant="secondary"
                  size="sm"
                  disabled={!!locked}
                  {...(locked ? { disabledReason: locked } : {})}
                  loading={clear.isPending}
                  onClick={() => clear.mutate()}
                >
                  {t("project.responseCache.clearButton", "Clear cache")}
                </Button>
              }
            />
          </>
        )}
      </SettingsGroup>
    </SettingsSection>
  );
}
