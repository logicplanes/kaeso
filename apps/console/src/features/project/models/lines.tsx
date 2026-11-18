import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Badge, Banner, Button, Empty, Input, KeyValue, List, Select, Text } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import type { LinePolicy, ModelLineView, OfferedLine } from "~/api/generated/types";
import { api, data } from "~/api/client";
import { projectCatalogue } from "~/api/queries/invalidation";
import { linesQuery, offeredLinesQuery } from "~/api/queries/models";
import { useApiMutation } from "~/auth/mutation";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

const DELAY_DEFAULT = 7;

export function policyOptions() {
  return [
    { value: "follow", label: t("models.lines.follow", "Follow automatically"), description: t("models.lines.followHint", "Moves when KPMR moves.") },
    { value: "ask", label: t("models.lines.ask", "Ask first"), description: t("models.lines.askHint", "Waits until someone moves or keeps it.") },
    { value: "delay", label: t("models.lines.delay", "Delay"), description: t("models.lines.delayHint", "Moves some days after KPMR moved.") },
    { value: "pin", label: t("models.lines.pin", "Pin"), description: t("models.lines.pinHint", "Stays until someone unpins it.") },
  ] satisfies { value: LinePolicy; label: string; description: string }[];
}

const isPolicy = (v: string): v is LinePolicy => v === "follow" || v === "ask" || v === "delay" || v === "pin";

function PolicyPicker({
  policy,
  days,
  onChange,
  disabled,
  disabledReason,
}: {
  policy: LinePolicy;
  days: number;
  onChange: (policy: LinePolicy, days: number) => void;
  disabled?: boolean;
  disabledReason?: string | undefined;
}) {
  return (
    <div className="flex flex-wrap items-end gap-8">
      <Select
        label={t("models.lines.policy", "When KPMR moves the line")}
        size="sm"
        value={policy}
        disabled={!!disabled}
        {...(disabledReason ? { disabledReason } : {})}
        onValueChange={(v) => isPolicy(v) && onChange(v, days)}
        options={policyOptions()}
      />
      {policy === "delay" && (
        <Input
          label={t("models.lines.delayDays", "Days to wait")}
          type="number"
          size="sm"
          className="w-96"
          min={1}
          max={30}
          disabled={!!disabled}
          value={String(days)}
          onChange={(e) => onChange("delay", Math.min(30, Math.max(1, Number(e.target.value) || 1)))}
        />
      )}
    </div>
  );
}

const body = (policy: LinePolicy, days: number) => ({ policy, ...(policy === "delay" ? { delay_days: days } : {}) });

export function OfferedLines({ org, project, provider, name }: { org: string; project: string; provider: string; name: string }) {
  const offered = useQuery(offeredLinesQuery(org, project, provider));
  const [policy, setPolicy] = useState<LinePolicy>("follow");
  const [days, setDays] = useState(DELAY_DEFAULT);
  const add = useApiMutation({
    mutationFn: async (line: OfferedLine) =>
      data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/lines", {
          params: { path: { org, project } },
          body: { provider: line.provider, line: line.line, ...body(policy, days) },
        }),
      ),
    invalidates: projectCatalogue(org, project),
    onSuccess: (l) => toast.success(t("models.lines.added", "{line} added; it points at {model}", { line: l.line, model: l.model })),
  });
  const list = offered.data?.data ?? [];
  return (
    <div className="flex flex-col gap-16">
      <PolicyPicker
        policy={policy}
        days={days}
        onChange={(p, d) => {
          setPolicy(p);
          setDays(d);
        }}
      />
      {offered.isPending ? (
        <List aria-label={t("models.lines.title", "Model lines")} items={[]} loading />
      ) : list.length === 0 ? (
        <Empty size="compact" title={t("models.lines.none", "{provider} has no model lines", { provider: name })} />
      ) : (
        <List
          aria-label={t("models.lines.title", "Model lines")}
          items={list.map((l) => ({
            id: l.line,
            textValue: l.line,
            title: <Text mono>{l.line}</Text>,
            subtitle: (
              <Text variant="small" tone="secondary" mono>
                {t("models.lines.pointsAt", "→ {model}", { model: l.model })}
              </Text>
            ),
            meta: l.added ? (
              <Badge>{t("models.lines.inCatalogue", "In the catalogue")}</Badge>
            ) : (
              <Button size="sm" loading={add.isPending && add.variables?.line === l.line} onClick={() => add.mutate(l)}>
                {t("models.lines.add", "Add line")}
              </Button>
            ),
          }))}
        />
      )}
    </div>
  );
}

function useLineAction(org: string, project: string, id: string, what: "move" | "keep" | "back", done: (l: ModelLineView) => string) {
  return useApiMutation({
    mutationFn: async () => {
      const options = { params: { path: { org, project, model: id } } };
      if (what === "move") return data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/lines/{model}/move", options));
      if (what === "keep") return data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/lines/{model}/keep", options));
      return data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/lines/{model}/back", options));
    },
    invalidates: projectCatalogue(org, project),
    onSuccess: (l) => toast.success(done(l)),
  });
}

export function LinePanel({
  org,
  project,
  id,
  canManage,
  reason,
}: {
  org: string;
  project: string;
  id: string;
  canManage: boolean;
  reason?: string | undefined;
}) {
  const lines = useQuery(linesQuery(org, project));
  const line = lines.data?.data.find((l) => l.id === id);
  const path = { org, project, model: id };
  const move = useLineAction(org, project, id, "move", (l) => t("models.lines.movedTo", "{line} points at {model} now", { line: l.line, model: l.model }));
  const keep = useLineAction(org, project, id, "keep", (l) => t("models.lines.kept", "{line} stays on {model}", { line: l.line, model: l.model }));
  const back = useLineAction(org, project, id, "back", (l) =>
    t("models.lines.backTo", "{line} is back on {model} and pinned", { line: l.line, model: l.model }),
  );
  const save = useApiMutation({
    mutationFn: async (next: { policy: LinePolicy; days: number }) =>
      data(await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/lines/{model}", { params: { path }, body: body(next.policy, next.days) })),
    invalidates: projectCatalogue(org, project),
    onSuccess: () => toast.success(t("models.lines.saved", "Line settings saved")),
  });
  if (!line) return null;
  const pending = line.pending;
  return (
    <div className="flex flex-col gap-12">
      <Text weight="medium">{t("models.lines.title", "Model lines")}</Text>
      {line.held && <Banner tone="warning">{line.held}</Banner>}
      {pending && (
        <Banner
          tone="info"
          title={t("models.lines.waits", "KPMR moved {line} to {model}", { line: line.line, model: pending.model })}
          actions={
            canManage ? (
              <>
                <Button size="sm" variant="primary" loading={move.isPending} onClick={() => move.mutate()}>
                  {t("models.lines.move", "Move")}
                </Button>
                {line.policy === "ask" && (
                  <Button size="sm" loading={keep.isPending} onClick={() => keep.mutate()}>
                    {t("models.lines.keep", "Keep")}
                  </Button>
                )}
              </>
            ) : undefined
          }
        >
          {pending.moves_at
            ? t("models.lines.movesAt", "It moves on {date}.", { date: formatDate(pending.moves_at) })
            : t("models.lines.waitsHint", "This project asks before the line moves.")}
        </Banner>
      )}
      <KeyValue
        items={[
          { label: t("models.lines.now", "Points at"), value: line.model, mono: true },
          { label: t("models.lines.since", "Since"), value: formatDate(line.since) },
          ...(line.previous ? [{ label: t("models.lines.previous", "Before"), value: line.previous, mono: true }] : []),
          ...(line.kpmr_model && line.kpmr_model !== line.model && !pending
            ? [{ label: t("models.lines.kpmr", "KPMR points at"), value: line.kpmr_model, mono: true }]
            : []),
        ]}
      />
      <PolicyPicker
        policy={line.policy}
        days={line.delay_days ?? DELAY_DEFAULT}
        disabled={!canManage || save.isPending}
        disabledReason={reason}
        onChange={(policy, days) => save.mutate({ policy, days })}
      />
      {canManage && line.previous && (
        <div>
          <Button size="sm" loading={back.isPending} onClick={() => back.mutate()}>
            {t("models.lines.moveBack", "Move back to {model}", { model: line.previous })}
          </Button>
        </div>
      )}
    </div>
  );
}
