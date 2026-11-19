import { useState } from "react";
import { Button, Checkbox, CheckboxGroup, CopyButton, Field, Input, Select, StepDialog, Switch, type StepDialogResult } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { SharedLinkSecret } from "~/api/generated/types";
import { sharedLinksKey } from "~/api/queries/shared-links";
import { useApiMutation } from "~/auth/mutation";
import { common } from "~/i18n/common";
import { t } from "~/i18n";

import { EXPIRIES, NEVER, expiryLabel, type ShareKind, type ShareTarget } from "./share-button";

const LONGEST = 400;

const days = (from: string, to: string) =>
  Math.min(LONGEST, Math.max(1, Math.round((Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / 86_400_000) + 1));

export function defaultTitle(kind: ShareKind, scope: string, hidden: boolean): string {
  if (kind === "member") return hidden ? t("kaeso.share.titleMemberHidden", "A member's usage") : t("kaeso.share.titleMember", "{scope}'s usage", { scope });
  if (kind === "budget") return hidden ? t("kaeso.share.titleBudgetHidden", "Budgets") : t("kaeso.share.titleBudget", "{scope} budgets", { scope });
  if (kind === "project_card") return hidden ? t("kaeso.share.titleCardHidden", "Project summary") : t("kaeso.share.titleCard", "{scope} summary", { scope });
  return hidden ? t("kaeso.share.titleUsageHidden", "Usage") : t("kaeso.share.titleUsage", "{scope} usage", { scope });
}

export default function ShareDialog({
  org,
  target,
  anyone,
  maxDays,
  onClose,
}: {
  org: string;
  target: ShareTarget & { scope: string };
  anyone: boolean;
  maxDays: number | null;
  onClose: () => void;
}) {
  const [title, setTitle] = useState<string | null>(null);
  const [live, setLive] = useState(true);
  const [costs, setCosts] = useState(false);
  const [hidden, setHidden] = useState<string[]>([]);
  const [audience, setAudience] = useState("members");
  const expiries: (number | typeof NEVER)[] = [
    ...EXPIRIES.filter((d) => maxDays === null || d <= maxDays),
    ...(maxDays === null ? [NEVER as typeof NEVER] : []),
  ];
  const [expires, setExpires] = useState<number | typeof NEVER>(Math.min(30, maxDays ?? 30));
  const [url, setUrl] = useState<string | null>(null);
  const hideScope = target.kind === "member" ? hidden.includes("members") : !!target.project && hidden.includes("projects");
  const shownTitle = title ?? defaultTitle(target.kind, target.scope, hideScope || !target.scope);
  const create = useApiMutation({
    mutationFn: async (): Promise<SharedLinkSecret> =>
      data(
        await api.POST("/platform/v1/orgs/{org}/shared-links", {
          params: { path: { org } },
          body: {
            kind: target.kind,
            ...(target.project ? { project: target.project } : {}),
            ...(target.member ? { member: target.member } : {}),
            title: shownTitle.trim(),
            live,
            ...(live ? { days: days(target.from, target.to) } : { from: target.from, to: target.to }),
            show_costs: costs,
            hide_members: hidden.includes("members"),
            hide_keys: hidden.includes("keys"),
            hide_projects: hidden.includes("projects"),
            audience,
            ...(expires === NEVER ? {} : { expires_in_days: expires }),
          },
        }),
      ),
    invalidates: [sharedLinksKey(org)],
    onSuccess: (r) => setUrl(r.url),
  });
  const hideOptions = [
    ...(target.kind === "project_card" ? [] : [{ value: "members", label: t("kaeso.share.hideMembers", "Member names") }]),
    ...(target.project && target.kind !== "project_card" ? [{ value: "keys", label: t("kaeso.share.hideKeys", "Key names") }] : []),
    target.project
      ? { value: "projects", label: t("kaeso.share.hideProject", "The project's name") }
      : { value: "projects", label: t("kaeso.share.hideProjects", "Project names") },
  ];
  const result: StepDialogResult | undefined = url
    ? {
        title: t("kaeso.share.made", "Link made"),
        description: t("kaeso.share.madeHint", "Copy the link now; Kaeso shows it only once and keeps only its hash."),
        footer: <Button onClick={onClose}>{common.finish()}</Button>,
        content: <CopyButton display="field" value={url} label={t("kaeso.share.copy", "Copy link")} />,
      }
    : undefined;
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      result={result}
      title={t("kaeso.share.title", "Share a read-only link")}
      submitLabel={t("kaeso.share.make", "Make link")}
      submitting={create.isPending}
      onSubmit={() => shownTitle.trim() && create.mutate()}
      steps={[
        {
          id: "what",
          title: t("kaeso.share.what", "What does it show?"),
          description: t("kaeso.share.whatHint", "Never request logs; what it hides stays hidden for everyone who opens it."),
          blockedReason: shownTitle.trim() ? undefined : t("kaeso.share.nameIt", "Give the link a title"),
          content: (
            <div className="flex flex-col gap-16">
              <Input label={t("kaeso.share.linkTitle", "Title")} value={shownTitle} onChange={(e) => setTitle(e.target.value)} maxLength={100} autoFocus />
              {target.kind !== "budget" && (
                <Select
                  label={t("kaeso.share.numbers", "Numbers")}
                  fullWidth
                  value={live ? "live" : "snapshot"}
                  onValueChange={(v) => setLive(v === "live")}
                  options={[
                    {
                      value: "live",
                      label: t("kaeso.share.live", "Live: always the last {count, plural, one {# day} other {# days}}", {
                        count: days(target.from, target.to),
                      }),
                    },
                    { value: "snapshot", label: t("kaeso.share.snapshot", "Snapshot: this period, as it is now") },
                  ]}
                />
              )}
              {target.costs && <Switch label={t("kaeso.share.showCosts", "Show costs")} checked={costs} onCheckedChange={setCosts} />}
              <Field label={t("kaeso.share.hide", "Hide")}>
                <CheckboxGroup aria-label={t("kaeso.share.hide", "Hide")} value={hidden} onValueChange={setHidden}>
                  {hideOptions.map((o) => (
                    <Checkbox key={o.value} value={o.value} label={o.label} />
                  ))}
                </CheckboxGroup>
              </Field>
            </div>
          ),
        },
        {
          id: "who",
          title: t("kaeso.share.who", "Who can open it?"),
          description: t("kaeso.share.whoHint", "You can revoke it any time under the organization's shared links."),
          content: (
            <div className="flex flex-col gap-16">
              <Select
                label={t("kaeso.share.audience", "Who")}
                fullWidth
                value={audience}
                onValueChange={setAudience}
                options={[
                  { value: "members", label: t("kaeso.share.members", "Members of this organization, signed in") },
                  {
                    value: "anyone",
                    label: t("kaeso.share.anyone", "Anyone with the link"),
                    disabled: !anyone,
                    ...(anyone ? {} : { description: t("kaeso.share.anyoneOff", "This organization allows links only for its members") }),
                  },
                ]}
              />
              <Select
                label={t("kaeso.share.expires", "Expires")}
                fullWidth
                value={String(expires)}
                onValueChange={(v) => setExpires(v === NEVER ? NEVER : Number(v))}
                options={expiries.map((d) => ({ value: String(d), label: expiryLabel(d) }))}
              />
            </div>
          ),
        },
      ]}
    />
  );
}
