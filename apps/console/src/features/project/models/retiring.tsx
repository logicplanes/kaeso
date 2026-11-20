import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Badge, Banner, Button, Empty, KeyValue, Link, List, Section, Sheet, Sparkline, Text } from "@logicplanes/kaeso-ui";
import { formatNumber } from "@logicplanes/kaeso-ui/format";
import type { Prices } from "~/format/prices";
import { useOrgPrices } from "~/kaeso/org-prices";
import type { CallerCount, MoveView, RetiringModelView } from "~/api/generated/types";
import { api, data } from "~/api/client";
import { projectCatalogue } from "~/api/queries/invalidation";
import { retiringQuery } from "~/api/queries/models";
import { useApiMutation } from "~/auth/mutation";
import { useProjectRights } from "~/auth/rights";
import { useRequestLogs } from "~/auth/session";
import { LoadFailed } from "~/features/org/load-failed";
import { fullDayLabel } from "~/format/period";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { toast } from "~/shell/toast";
import { allKeysQuery } from "~/api/queries/keys";
import { clientLabel } from "~/format/clients";
import { keyLabel, usePartyName } from "../keys/party";

const SOON_DAYS = 90;

export function countdown(days: number): string {
  if (days < 0) return t("models.retiring.daysAgo", "{count, plural, one {retired # day ago} other {retired # days ago}}", { count: -days });
  if (days === 0) return t("models.retiring.today", "shuts down today");
  return t("models.retiring.inDays", "{count, plural, one {in # day} other {in # days}}", { count: days });
}

export function stateBadge(m: Pick<RetiringModelView, "state">) {
  if (m.state === "retired") return <Badge tone="danger">{t("models.retiring.retired", "Retired")}</Badge>;
  if (m.state === "deprecated") return <Badge tone="warning">{t("models.deprecated", "Deprecated")}</Badge>;
  return <Badge>{t("models.retiring.announced", "Retirement announced")}</Badge>;
}

/** What the catalogue's row says about a retiring model: when it shuts down, or that it has. */
export function shutdownLabel(m: Pick<RetiringModelView, "lifecycle" | "days_left" | "state">): string | null {
  if (m.state === "retired") return t("models.retiring.retired", "Retired");
  const day = m.lifecycle.shutdown_on;
  return day ? t("models.retiring.shutsDownOn", "Shuts down {date}", { date: fullDayLabel(day) }) : null;
}

export function RetiringBanner({ count, onShow }: { count: number; onShow: () => void }) {
  if (count === 0) return null;
  return (
    <Banner
      tone="warning"
      title={t("models.retiring.bannerTitle", "{count, plural, one {# model shuts down soon} other {# models shut down soon}}", { count })}
      actions={
        <Button size="sm" onClick={onShow}>
          {t("models.retiring.show", "Show retiring models")}
        </Button>
      }
    >
      {t("models.retiring.bannerHint", "Their providers retire them within {days} days; move apps and aliases to the replacements.", { days: SOON_DAYS })}
    </Banner>
  );
}

function priceChange(prices: Prices, from: string | null | undefined, to: string | null | undefined): string {
  const price = (v: string | null | undefined) => (v === null || v === undefined ? t("models.retiring.unknownPrice", "unknown") : prices.unit(v));
  return t("models.retiring.priceChange", "{from} → {to}", { from: price(from), to: price(to) });
}

export function RetiringTab({ org, project, canManage }: { org: string; project: string; canManage: boolean }) {
  const retiring = useQuery(retiringQuery(org, project));
  const [open, setOpen] = useState<string | null>(null);
  const list = retiring.data?.data ?? [];
  const shown = list.find((m) => m.id === open);
  return (
    <>
      {retiring.isPending ? (
        <List aria-label={t("models.retiring.title", "Retiring")} items={[]} loading />
      ) : retiring.isError ? (
        <LoadFailed title={t("models.retiring.loadFailed", "The retiring models couldn't load")} />
      ) : list.length === 0 ? (
        <Empty
          size="lg"
          title={t("models.retiring.empty", "No model of this project is retiring")}
          description={t("models.retiring.emptyHint", "When a provider announces a shutdown date for one of them, it shows here.")}
        />
      ) : (
        <List
          aria-label={t("models.retiring.title", "Retiring")}
          onOpen={(item) => setOpen(item.id)}
          items={list.map((m) => ({
            id: m.id,
            textValue: m.model_id,
            title: <Text mono>{`${m.provider}/${m.model_id}`}</Text>,
            subtitle: (
              <Text variant="small" tone="secondary" truncate>
                {m.lifecycle.replacement
                  ? t("models.retiring.replaceWith", "Replace with {model}", { model: m.lifecycle.replacement })
                  : t("models.retiring.noReplacement", "No replacement named")}
              </Text>
            ),
            meta: (
              <span className="flex items-center gap-8">
                {stateBadge(m)}
                {m.usage && (
                  <Text variant="small" tone="secondary" tabular>
                    {t("models.retiring.requests30", "{count, plural, one {# request in 30 days} other {# requests in 30 days}}", { count: m.usage.requests })}
                  </Text>
                )}
              </span>
            ),
            ...(m.days_left === null || m.days_left === undefined ? {} : { date: countdown(m.days_left) }),
          }))}
        />
      )}
      <Sheet open={!!shown} onOpenChange={(o) => !o && setOpen(null)} modal={false} size="md" title={shown ? shown.model_id : ""}>
        {shown && <RetiringBody key={shown.id} org={org} project={project} m={shown} canManage={canManage} onMoved={() => setOpen(null)} />}
      </Sheet>
    </>
  );
}

function Callers({ title, list, name, href }: { title: string; list: CallerCount[]; name: (id: string) => string; href: (id: string) => string | undefined }) {
  if (list.length === 0) return null;
  return (
    <KeyValue
      layout="stacked"
      items={[
        {
          label: title,
          value: (
            <ul className="flex flex-col gap-4">
              {list.slice(0, 8).map((c) => {
                const to = href(c.id);
                return (
                  <li key={c.id} className="flex items-center justify-between gap-12">
                    {to ? (
                      <Link href={to} className={name(c.id) === c.id ? "mono truncate" : "truncate"}>
                        {name(c.id)}
                      </Link>
                    ) : (
                      <Text truncate mono={name(c.id) === c.id}>
                        {name(c.id)}
                      </Text>
                    )}
                    <Text tabular tone="secondary">
                      {formatNumber(c.requests)}
                    </Text>
                  </li>
                );
              })}
            </ul>
          ),
        },
      ]}
    />
  );
}

function RetiringBody({
  org,
  project,
  m,
  canManage,
  onMoved,
}: {
  org: string;
  project: string;
  m: RetiringModelView;
  canManage: boolean;
  onMoved: () => void;
}) {
  const rights = useProjectRights(org, project);
  const prices = useOrgPrices(org);
  const logs = useRequestLogs() && rights.has("logs.metadata.view");
  const [moving, setMoving] = useState(false);
  const partyName = usePartyName(org, project);
  const keys = useQuery({ ...allKeysQuery(org, project, { include_archived: true }), enabled: rights.has("keys.all.view") });
  const keyName = (id: string) => {
    const k = keys.data?.find((x) => x.id === id);
    return k ? keyLabel(k) : id;
  };
  const base = `/orgs/${org}/projects/${project}`;
  const logsOf = (filter: Record<string, string>) =>
    logs ? `${base}/logs?${new URLSearchParams({ model: m.model_id, provider: m.provider, range: "30d", ...filter }).toString()}` : undefined;
  const l = m.lifecycle;
  const r = m.replacement;
  const usage = m.usage;
  const movable = !!r && (r.listed || !!r.catalogue_model_id) && m.aliases.length > 0;
  const moveBlocked = movable
    ? undefined
    : m.aliases.length === 0
      ? t("models.retiring.nothingToMove", "Nothing to move; no alias calls it.")
      : t("models.retiring.replacementUnlisted", "KPMR doesn't list the replacement, so it can't be added.");
  return (
    <div className="flex flex-col gap-24">
      <div className="flex flex-wrap items-center gap-8">
        {stateBadge(m)}
        {m.days_left !== null && m.days_left !== undefined && (
          <Text variant="small" tone="secondary">
            {countdown(m.days_left)}
          </Text>
        )}
      </div>
      <KeyValue
        items={[
          ...(l.announced_on ? [{ label: t("models.retiring.announcedOn", "Announced"), value: fullDayLabel(l.announced_on) }] : []),
          ...(l.deprecated_on ? [{ label: t("models.retiring.deprecatedOn", "Deprecated"), value: fullDayLabel(l.deprecated_on) }] : []),
          ...(l.shutdown_on ? [{ label: t("models.retiring.shutdownOn", "Shuts down"), value: fullDayLabel(l.shutdown_on) }] : []),
          { label: t("models.retiring.replacement", "Replacement"), value: l.replacement ?? common.none(), mono: !!l.replacement },
          ...(r
            ? [
                { label: t("models.retiring.inputPrice", "Input price"), value: priceChange(prices, r.input_price.from, r.input_price.to) },
                { label: t("models.retiring.outputPrice", "Output price"), value: priceChange(prices, r.output_price.from, r.output_price.to) },
              ]
            : []),
          ...(l.source
            ? [
                {
                  label: t("models.retiring.source", "Announcement"),
                  value: (
                    <Link href={l.source} external className="wrap-anywhere">
                      {new URL(l.source).host}
                    </Link>
                  ),
                },
              ]
            : []),
        ]}
      />
      <Section
        title={t("models.retiring.usedBy", "Aliases and route steps")}
        spacing="none"
        actions={
          canManage &&
          r && (
            <Button size="sm" variant="primary" disabled={!movable} {...(moveBlocked ? { disabledReason: moveBlocked } : {})} onClick={() => setMoving(true)}>
              {t("models.retiring.move", "Move to replacement")}
            </Button>
          )
        }
      >
        {m.aliases.length === 0 ? (
          <Text tone="secondary">{t("models.retiring.noAliases", "No alias calls it; apps that name it directly still do.")}</Text>
        ) : (
          <ul className="flex flex-col gap-4">
            {m.aliases.map((a) => (
              <li key={a.alias_id} className="flex flex-wrap items-baseline gap-8">
                <Link href={`${base}/models/aliases/${a.alias_id}`} className="mono">
                  {a.name}
                </Link>
                <Text variant="small" tone="secondary" mono>
                  {a.steps.join(", ")}
                </Text>
              </li>
            ))}
          </ul>
        )}
      </Section>
      {usage && (
        <Section title={t("models.retiring.callers", "Who still calls it, last 30 days")} spacing="none">
          <div className="flex flex-col gap-12">
            {usage.daily.length > 1 && (
              <Sparkline values={usage.daily.map((d) => d.requests)} fluid aria-label={t("models.retiring.chart", "Requests per day since the announcement")} />
            )}
            {usage.requests === 0 ? (
              <Text tone="secondary">{t("models.retiring.noCalls", "No requests in the last 30 days.")}</Text>
            ) : (
              <>
                <Callers title={t("models.retiring.keys", "Keys")} list={usage.keys} name={keyName} href={(id) => logsOf({ key: id })} />
                <Callers
                  title={t("models.retiring.members", "Members")}
                  list={usage.members}
                  name={(id) => partyName({ kind: "user", id })}
                  href={(id) => logsOf({ member: `user:${id}` })}
                />
                <Callers
                  title={t("models.retiring.serviceAccounts", "Service accounts")}
                  list={usage.service_accounts}
                  name={(id) => partyName({ kind: "service_account", id })}
                  href={(id) => logsOf({ member: `service_account:${id}` })}
                />
                <Callers title={t("models.retiring.clients", "Client apps")} list={usage.clients} name={clientLabel} href={(id) => logsOf({ client: id })} />
              </>
            )}
          </div>
        </Section>
      )}
      {r && <MoveDialog org={org} project={project} m={m} to={r.model_id} open={moving} onOpenChange={setMoving} onMoved={onMoved} />}
    </div>
  );
}

function MoveDialog({
  org,
  project,
  m,
  to,
  open,
  onOpenChange,
  onMoved,
}: {
  org: string;
  project: string;
  m: RetiringModelView;
  to: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onMoved: () => void;
}) {
  const path = { org, project, model: m.id };
  const preview = useQuery({
    queryKey: ["orgs", org, "projects", project, "deprecations", m.id, "move"],
    queryFn: async (): Promise<MoveView> => data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/deprecations/{model}/move", { params: { path } })),
    enabled: open,
  });
  const move = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/deprecations/{model}/move", { params: { path } })),
    invalidates: projectCatalogue(org, project),
    onSuccess: (done) => {
      toast.success(t("models.retiring.moved", "{model} moved to {replacement}", { model: m.model_id, replacement: to }), {
        description: t("models.retiring.movedSteps", "{count, plural, one {# route step changed.} other {# route steps changed.}}", { count: done.steps }),
      });
      onOpenChange(false);
      onMoved();
    },
  });
  const p = preview.data;
  return (
    <AlertDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("models.retiring.moveTitle", "Move {model} to {replacement}?", { model: m.model_id, replacement: to })}
      description={
        p
          ? [
              t(
                "models.retiring.moveSteps",
                "{count, plural, one {# route step in {aliases} points at the replacement.} other {# route steps in {aliases} point at the replacement.}}",
                {
                  count: p.steps,
                  aliases: m.aliases.map((a) => a.name).join(", "),
                },
              ),
              p.adds_replacement ? t("models.retiring.moveAdds", "The replacement is added to the catalogue with KPMR's prices.") : "",
            ]
              .filter(Boolean)
              .join(" ")
          : t("models.retiring.movePreview", "Checking what changes…")
      }
      tone="warning"
      confirmLabel={t("models.retiring.move", "Move to replacement")}
      loading={move.isPending}
      onConfirm={() => move.mutateAsync()}
    />
  );
}
