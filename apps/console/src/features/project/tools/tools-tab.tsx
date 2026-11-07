import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AppFrame, Badge, Banner, Button, Code, Empty, FilterChip, Input, List, Status, Switch, Text, Toolbar, type ListGroup } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { ProjectToolView } from "~/api/generated/types";
import { projectToolViews } from "~/api/queries/invalidation";
import { projectServersQuery, projectToolsQuery } from "~/api/queries/tools";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useProjectRights } from "~/auth/rights";
import { LoadFailed } from "~/features/org/load-failed";
import { t } from "~/i18n";
import { HINTS, hintLabel, hintsOf, projectServerWord } from "~/kaeso/mcp-servers";
import type { SearchPatch } from "~/search";
import { toast } from "~/shell/toast";
import type { ToolState, ToolsSearch } from "./tools-page";

const STATES: ToolState[] = ["on", "off", "changed"];

export function stateOf(tool: ProjectToolView): ToolState {
  if (tool.state === "changed") return "changed";
  return tool.enabled ? "on" : "off";
}

export function stateWord(state: ToolState): { tone: "success" | "warning" | "neutral"; label: string } {
  if (state === "changed") return { tone: "warning", label: t("project.tools.state.changed", "Changed") };
  if (state === "on") return { tone: "success", label: t("project.tools.state.on", "On") };
  return { tone: "neutral", label: t("project.tools.state.off", "Off") };
}

const matches = (tool: ProjectToolView, words: string[]) =>
  words.every((w) => [tool.exposed_name, tool.name, tool.title ?? "", tool.description].some((f) => f.toLocaleLowerCase().includes(w)));

export function ToolsTab({
  org,
  project,
  search,
  setSearch,
}: {
  org: string;
  project: string;
  search: ToolsSearch;
  setSearch: (patch: SearchPatch<ToolsSearch>) => void;
}) {
  const rights = useProjectRights(org, project);
  const needs = useNeedsRight(org, project);
  const canManage = rights.has("tools.manage");
  const reason = canManage ? undefined : needs("tools.manage");
  const tools = useQuery(projectToolsQuery(org, project));
  const servers = useQuery(projectServersQuery(org, project));
  const [query, setQuery] = useState("");
  const all = useMemo(() => (tools.data ?? []).filter((x) => x.state !== "removed"), [tools.data]);
  const shown = useMemo(() => {
    const words = query.toLocaleLowerCase().split(/\s+/).filter(Boolean);
    return all.filter(
      (x) =>
        (!search.server || x.server === search.server) &&
        (!search.state || stateOf(x) === search.state) &&
        (!search.hint || hintsOf(x.annotations).includes(search.hint)) &&
        matches(x, words),
    );
  }, [all, query, search.server, search.state, search.hint]);
  const held = all.filter((x) => x.state === "changed").length;

  const toggle = useApiMutation({
    mutationFn: async ({ tool, enabled }: { tool: ProjectToolView; enabled: boolean }) =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/tools", {
          params: { path: { org, project } },
          body: { tools: [{ id: tool.id, enabled }] },
        }),
      ),
    invalidates: [projectToolViews(org)],
    onSuccess: (_list, { tool, enabled }) =>
      toast(
        enabled
          ? t("project.tools.turnedOn", "{tool} is on", { tool: tool.exposed_name })
          : t("project.tools.turnedOff", "{tool} is off", { tool: tool.exposed_name }),
      ),
  });

  const toggleAll = useApiMutation({
    mutationFn: async ({ tools: list, enabled }: { server: string; tools: ProjectToolView[]; enabled: boolean }) =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/tools", {
          params: { path: { org, project } },
          body: { tools: list.map((x) => ({ id: x.id, enabled })) },
        }),
      ),
    invalidates: [projectToolViews(org)],
    onSuccess: (_list, { server, tools: list, enabled }) =>
      toast(
        enabled
          ? t("project.tools.turnedAllOn", "{count, plural, one {# tool} other {# tools}} of {server} on", { count: list.length, server })
          : t("project.tools.turnedAllOff", "{count, plural, one {# tool} other {# tools}} of {server} off", { count: list.length, server }),
      ),
  });

  const groups: ListGroup[] = (servers.data ?? [])
    .map((s) => {
      const items = shown.filter((x) => x.server === s.id);
      const word = projectServerWord(s);
      const switchable = items.filter((x) => x.state === "active");
      const off = switchable.filter((x) => !x.enabled);
      const on = switchable.filter((x) => x.enabled);
      const bulk = (list: ProjectToolView[], enabled: boolean) => toggleAll.mutate({ server: s.name, tools: list, enabled });
      return {
        id: s.id,
        ...(canManage && switchable.length > 1
          ? {
              actions: (
                <>
                  {off.length > 0 && (
                    <Button variant="ghost" size="xs" disabled={toggleAll.isPending} onClick={() => bulk(off, true)}>
                      {t("project.tools.allOn", "Turn all on")}
                    </Button>
                  )}
                  {on.length > 0 && (
                    <Button variant="ghost" size="xs" disabled={toggleAll.isPending} onClick={() => bulk(on, false)}>
                      {t("project.tools.allOff", "Turn all off")}
                    </Button>
                  )}
                </>
              ),
            }
          : {}),
        title: (
          <span className="flex min-w-0 items-center gap-12">
            <Text weight="medium" truncate>
              {s.name}
            </Text>
            <Status tone={word.tone}>{word.label}</Status>
            <Text variant="small" tone="secondary">
              {t("project.tools.onCount", "{on} of {total} on", { on: s.tools_on, total: s.tools })}
            </Text>
          </span>
        ),
        count: items.length,
        items: items.map((x) => {
          const state = stateWord(stateOf(x));
          const label = x.title || x.description;
          return {
            id: x.id,
            textValue: x.exposed_name,
            leading: (
              <Switch
                size="sm"
                aria-label={t("project.tools.offer", "Offer {tool}", { tool: x.exposed_name })}
                checked={x.enabled}
                disabled={!canManage || x.state !== "active"}
                {...(reason
                  ? { disabledReason: reason }
                  : x.state !== "active"
                    ? { disabledReason: t("project.tools.waitsForReview", "Waits for review") }
                    : {})}
                onCheckedChange={(enabled) => toggle.mutate({ tool: x, enabled })}
              />
            ),
            title: <Code>{x.exposed_name}</Code>,
            ...(label ? { subtitle: label } : {}),
            meta: (
              <span className="flex items-center gap-8">
                {hintsOf(x.annotations).map((h) => (
                  <Badge key={h} tone={h === "destructive" ? "warning" : "outline"}>
                    {hintLabel(h)}
                  </Badge>
                ))}
                <Status tone={state.tone}>{state.label}</Status>
              </span>
            ),
            ...(x.calls_7d != null ? { date: t("project.tools.calls", "{count, plural, one {# call} other {# calls}}", { count: x.calls_7d }) } : {}),
          };
        }),
      };
    })
    .filter((g) => g.items.length > 0);

  const serverName = (id: string) => servers.data?.find((s) => s.id === id)?.name ?? id;
  const filtering = !!(query || search.server || search.state || search.hint);
  return (
    <>
      <Toolbar aria-label={t("project.tools.filter", "Filter tools")}>
        <div className="w-240">
          <Input
            type="search"
            size="sm"
            aria-label={t("project.tools.search", "Search tools")}
            placeholder={t("project.tools.searchPlaceholder", "Tool or description")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onClear={() => setQuery("")}
          />
        </div>
        <FilterChip
          property={t("project.tools.server", "Server")}
          operator={t("logs.filter.is", "is")}
          value={search.server ? serverName(search.server) : t("logs.filter.any", "any")}
          values={(servers.data ?? []).map((s) => ({ value: s.id, label: s.name }))}
          selected={search.server ? [search.server] : []}
          onSelectedChange={(v) => setSearch({ server: v[0] })}
          {...(search.server ? { onRemove: () => setSearch({ server: undefined }) } : {})}
        />
        <FilterChip
          property={t("project.tools.state", "State")}
          operator={t("logs.filter.is", "is")}
          value={search.state ? stateWord(search.state).label : t("logs.filter.any", "any")}
          values={STATES.map((s) => ({ value: s, label: stateWord(s).label }))}
          selected={search.state ? [search.state] : []}
          onSelectedChange={(v) => setSearch({ state: STATES.find((s) => s === v[0]) })}
          {...(search.state ? { onRemove: () => setSearch({ state: undefined }) } : {})}
        />
        <FilterChip
          property={t("project.tools.hint", "Hint")}
          operator={t("logs.filter.is", "is")}
          value={search.hint ? hintLabel(search.hint) : t("logs.filter.any", "any")}
          values={HINTS.map((h) => ({ value: h, label: hintLabel(h) }))}
          selected={search.hint ? [search.hint] : []}
          onSelectedChange={(v) => setSearch({ hint: HINTS.find((h) => h === v[0]) })}
          {...(search.hint ? { onRemove: () => setSearch({ hint: undefined }) } : {})}
        />
      </Toolbar>
      <AppFrame.Body padding="list">
        {held > 0 && (
          <Banner tone="warning" title={t("project.tools.heldTitle", "{count, plural, one {# tool waits} other {# tools wait}} for review", { count: held })}>
            {t("project.tools.heldHint", "The portal offers a changed tool again once an admin approves the change.")}
          </Banner>
        )}
        {tools.isPending || servers.isPending ? (
          <List aria-label={t("project.tools.title", "Tools")} items={[]} loading />
        ) : tools.isError || servers.isError ? (
          <LoadFailed title={t("project.tools.loadFailed", "The tools couldn't load")} />
        ) : groups.length === 0 ? (
          <Empty
            size="lg"
            title={filtering ? t("project.tools.noMatches", "No tools match") : t("project.tools.empty", "No tools yet")}
            description={
              filtering
                ? t("project.tools.noMatchesHint", "Clear the search and filters to see every tool.")
                : t("project.tools.emptyHint", "Use one of your organization's MCP servers to offer its tools here.")
            }
            actions={
              filtering ? (
                <Button
                  onClick={() => {
                    setQuery("");
                    setSearch({ server: undefined, state: undefined, hint: undefined });
                  }}
                >
                  {t("project.tools.clearFilters", "Clear filters")}
                </Button>
              ) : (
                <Button variant="primary" onClick={() => setSearch({ tab: "servers" })}>
                  {t("project.tools.useServer", "Use a server")}
                </Button>
              )
            }
          />
        ) : (
          <List aria-label={t("project.tools.title", "Tools")} groups={groups} onOpen={(item) => setSearch({ tool: item.id })} />
        )}
      </AppFrame.Body>
    </>
  );
}
