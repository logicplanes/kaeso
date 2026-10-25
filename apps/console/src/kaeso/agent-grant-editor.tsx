import { useState, type ReactNode } from "react";
import { Button, Checkbox, Collapsible, Dialog, RightsMatrix, Segmented, Switch, Tabs, type RightChange, type RightsGroup } from "@logicplanes/kaeso-ui";
import type { AgentCatalogue, Grantable, GrantableOrg } from "~/api/generated/types";
import { t } from "~/i18n";
import { heldInProjects, presetDraft, presetPart, sorted, type AgentDraft, type OrgDraft, type PresetId } from "./agent-grant";

type Scope = "instance" | "account" | "org" | "project";

function groups(cat: AgentCatalogue, scope: Scope, granted: readonly string[], held: readonly string[], locked?: string): RightsGroup[] {
  const on = new Set(granted);
  const can = new Set(held);
  const never = new Set(cat.never_grantable);
  return cat[scope].categories.map((c) => ({
    label: c.name,
    rights: c.rights.map((r) => {
      const reason =
        locked ??
        (never.has(r.id)
          ? t("kaeso.agents.neverGrantable", "Agents can't be given this right")
          : !can.has(r.id) && !on.has(r.id)
            ? t("kaeso.rights.notHeld", "You don't hold this right, so you can't grant it")
            : undefined);
      return {
        code: r.id,
        label: r.name,
        description: r.description,
        granted: on.has(r.id),
        sensitive: r.sensitive,
        requires: r.needs,
        ...(reason ? { disabled: true, disabledReason: reason } : {}),
      };
    }),
  }));
}

function applied(cat: AgentCatalogue, scope: Scope, current: readonly string[], changes: readonly RightChange[]): string[] {
  const next = new Set(current);
  for (const c of changes) {
    if (c.granted) next.add(c.code);
    else next.delete(c.code);
  }
  return sorted(cat, scope, next);
}

export function PresetPicker({
  cat,
  grantable,
  value,
  onChange,
}: {
  cat: AgentCatalogue;
  grantable: Grantable;
  value: AgentDraft;
  onChange: (d: AgentDraft) => void;
}) {
  return (
    <Segmented
      aria-label={t("kaeso.agents.preset", "Preset")}
      fullWidth
      size="sm"
      options={cat.presets.map((p) => ({ value: p.id, label: p.name }))}
      value={value.customized ? "" : value.preset}
      onValueChange={(id) => {
        const next = presetDraft(cat, grantable, id as PresetId);
        onChange({ ...next, future: { ...next.future, on: value.future.on } });
      }}
    />
  );
}

function OrgRights({ cat, org, value, onChange }: { cat: AgentCatalogue; org: GrantableOrg; value: OrgDraft; onChange: (o: OrgDraft) => void }) {
  return (
    <div className="flex flex-col gap-16">
      <RightsMatrix
        searchable
        aria-label={t("kaeso.agents.orgRights", "Rights in {org}", { org: org.name })}
        groups={groups(cat, "org", value.rights, org.rights)}
        onRightsChange={(c) => onChange({ ...value, rights: applied(cat, "org", value.rights, c) })}
      />
      <Segmented
        aria-label={t("kaeso.agents.projects", "Projects")}
        size="sm"
        options={[
          { value: "all", label: t("kaeso.agents.allProjects", "All projects") },
          { value: "selected", label: t("kaeso.agents.selectedProjects", "Selected projects") },
        ]}
        value={value.all ? "all" : "selected"}
        onValueChange={(v) => onChange({ ...value, all: v === "all" })}
      />
      {value.all ? (
        <RightsMatrix
          aria-label={t("kaeso.agents.allProjectRights", "Rights in every project of {org}", { org: org.name })}
          groups={groups(cat, "project", value.projectRights, heldInProjects(org))}
          onRightsChange={(c) => onChange({ ...value, projectRights: applied(cat, "project", value.projectRights, c) })}
        />
      ) : (
        <div className="flex flex-col gap-12">
          {org.projects.map((p) => {
            const chosen = value.selected[p.id];
            return (
              <div key={p.id} className="flex flex-col gap-8">
                <Checkbox
                  label={p.name}
                  checked={!!chosen}
                  onCheckedChange={(on) => {
                    const selected = Object.fromEntries(Object.entries(value.selected).filter(([id]) => id !== p.id));
                    if (on) selected[p.id] = value.projectRights.filter((r) => p.rights.includes(r));
                    onChange({ ...value, selected });
                  }}
                />
                {chosen && (
                  <RightsMatrix
                    aria-label={t("kaeso.agents.projectRights", "Rights in {project}", { project: p.name })}
                    groups={groups(cat, "project", chosen, p.rights)}
                    onRightsChange={(c) => onChange({ ...value, selected: { ...value.selected, [p.id]: applied(cat, "project", chosen, c) } })}
                  />
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

export function AgentGrantEditor({
  cat,
  grantable,
  value,
  onChange,
  open,
  onOpenChange,
  title,
  description,
  start,
  children,
}: {
  cat: AgentCatalogue;
  grantable: Grantable;
  value: AgentDraft;
  onChange: (d: AgentDraft) => void;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  start?: "organizations";
  children?: ReactNode;
}) {
  const [tab, setTab] = useState("organizations");
  const edit = (d: Partial<AgentDraft>) => onChange({ ...value, ...d, customized: true });
  const editOrg = (id: string, o: OrgDraft) => edit({ orgs: { ...value.orgs, [id]: o } });
  const presetRights = (part: "org" | "project") => presetPart(cat, value.preset, part);
  const instancePresets = cat.instance_presets.filter((p) => p.rights.every((r) => grantable.instance.includes(r)));
  const instancePreset = instancePresets.find((p) => p.rights.length === value.instance.length && p.rights.every((r) => value.instance.includes(r)));
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      description={description}
      size="lg"
      anchor="top"
      footer={
        <Dialog.Close>
          <Button variant="primary">{t("common.done", "Done")}</Button>
        </Dialog.Close>
      }
    >
      {children && <div className="mb-24">{children}</div>}
      <Tabs value={tab} onValueChange={setTab}>
        <Tabs.List>
          <Tabs.Tab value="organizations">{t("kaeso.agents.organizations", "Organizations")}</Tabs.Tab>
          {grantable.instance.length > 0 && <Tabs.Tab value="instance">{t("kaeso.agents.instance", "Instance")}</Tabs.Tab>}
          <Tabs.Tab value="account">{t("kaeso.agents.account", "Account")}</Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="organizations">
          <div className="mt-24 flex flex-col gap-16">
            {grantable.orgs.map((o) => {
              const draft = value.orgs[o.id] ?? { included: false, rights: [], all: true, projectRights: [], selected: {} };
              return (
                <div key={o.id} className="flex flex-col gap-12">
                  <Checkbox
                    label={o.name}
                    description={o.allowed ? undefined : t("kaeso.agents.orgClosed", "This organization doesn't allow agents.")}
                    disabled={!o.allowed}
                    checked={draft.included}
                    onCheckedChange={(on) =>
                      editOrg(
                        o.id,
                        on && !draft.rights.length
                          ? {
                              ...draft,
                              included: true,
                              rights: presetRights("org").filter((r) => o.rights.includes(r)),
                              projectRights: presetRights("project").filter((r) => heldInProjects(o).includes(r)),
                            }
                          : { ...draft, included: on },
                      )
                    }
                  />
                  {draft.included && (
                    <Collapsible
                      title={t("kaeso.agents.rightsIn", "Rights in {org}", { org: o.name })}
                      hint={t("kaeso.rights.count", "{count, plural, one {# right} other {# rights}}", {
                        count: draft.rights.length + draft.projectRights.length,
                      })}
                      defaultOpen={start === "organizations"}
                    >
                      <OrgRights cat={cat} org={o} value={draft} onChange={(next) => editOrg(o.id, next)} />
                    </Collapsible>
                  )}
                </div>
              );
            })}
            <Switch
              label={t("kaeso.agents.futureOrgs", "Also orgs I join later")}
              description={t("kaeso.agents.futureOrgsHint", "With the preset's rights, never more than you hold there.")}
              checked={value.future.on}
              onCheckedChange={(on) => edit({ future: { on, rights: presetRights("org"), projectRights: presetRights("project") } })}
            />
          </div>
        </Tabs.Panel>
        {grantable.instance.length > 0 && (
          <Tabs.Panel value="instance">
            <div className="mt-24 flex flex-col gap-16">
              <Segmented
                aria-label={t("kaeso.agents.instancePreset", "Instance preset")}
                size="sm"
                options={instancePresets.map((p) => ({ value: p.id, label: p.name }))}
                value={instancePreset?.id ?? ""}
                onValueChange={(id) => edit({ instance: instancePresets.filter((p) => p.id === id).flatMap((p) => p.rights) })}
              />
              <RightsMatrix
                aria-label={t("kaeso.agents.instanceRights", "Rights on the instance")}
                groups={groups(cat, "instance", value.instance, grantable.instance)}
                onRightsChange={(c) => edit({ instance: applied(cat, "instance", value.instance, c) })}
              />
            </div>
          </Tabs.Panel>
        )}
        <Tabs.Panel value="account">
          <div className="mt-24 flex flex-col gap-16">
            <RightsMatrix
              aria-label={t("kaeso.agents.accountRights", "Rights on your account")}
              groups={groups(
                cat,
                "account",
                value.account,
                grantable.account.filter((r) => r !== "account.view"),
              )}
              onRightsChange={(c) => edit({ account: sorted(cat, "account", ["account.view", ...applied(cat, "account", value.account, c)]) })}
            />
          </div>
        </Tabs.Panel>
      </Tabs>
    </Dialog>
  );
}
