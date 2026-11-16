import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, Input, Section, Segmented, Select, SettingsGroup, SettingsRow, Text } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { GroupRule, GroupRuleProject, SsoGroups as SsoGroupsView } from "~/api/generated/types";
import { orgProjectsQuery } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { rolesOfScope, useRoles } from "~/kaeso/roles";
import { toast } from "~/shell/toast";

const NO_ROLE = "none";

type Rule = GroupRule & { projects: GroupRuleProject[] };

const groupsQuery = (org: string) => ({
  queryKey: ["orgs", org, "security", "sso", "groups"] as const,
  queryFn: async (): Promise<SsoGroupsView> => data(await api.GET("/platform/v1/orgs/{org}/security/sso/groups", { params: { path: { org } } })),
});

/** Security › Single sign-on › Groups: roles and projects from the identity provider's groups. */
export function SsoGroups({ org }: { org: string }) {
  const groups = useQuery(groupsQuery(org));
  return groups.data ? <GroupsEditor org={org} initial={groups.data} /> : null;
}

function GroupsEditor({ org, initial }: { org: string; initial: SsoGroupsView }) {
  const needs = useNeedsRight(org);
  const canManage = useOrgRights(org).has("security.sso.manage");
  const reason = canManage ? undefined : needs("security.sso.manage");
  const [claim, setClaim] = useState(initial.claim);
  const [scopes, setScopes] = useState(initial.scopes.join(" "));
  const [mode, setMode] = useState(initial.mode);
  const [rules, setRules] = useState<Rule[]>(() => initial.rules.map((r) => ({ ...r, projects: r.projects ?? [] })));
  const roles = useRoles(org);
  const orgRoles = rolesOfScope(roles, "org").filter((r) => r.id !== "owner");
  const projectRoles = rolesOfScope(roles, "project");
  const projects = useQuery(orgProjectsQuery(org)).data ?? [];
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PUT("/platform/v1/orgs/{org}/security/sso/groups", {
          params: { path: { org } },
          body: { claim, scopes: scopes.split(/\s+/).filter(Boolean), mode, rules },
        }),
      ),
    invalidates: [["orgs", org, "security", "sso", "groups"]],
    onSuccess: () => toast.success(t("org.ssoGroups.saved", "Group rules saved")),
  });
  const edit = (i: number, rule: Rule) => setRules(rules.map((r, j) => (j === i ? rule : r)));
  const editProject = (i: number, rule: Rule, k: number, change: Partial<GroupRuleProject>) =>
    edit(i, { ...rule, projects: rule.projects.map((q, m) => (m === k ? { ...q, ...change } : q)) });
  const firstProject = projects.at(0);
  return (
    <Section
      spacing="settings"
      title={t("org.ssoGroups.title", "Groups")}
      description={t("org.ssoGroups.hint", "Rules give roles and projects from the provider's groups at each sign-in.")}
    >
      <SettingsGroup>
        <SettingsRow
          title={t("org.ssoGroups.claim", "Groups claim")}
          control={
            <Input aria-label={t("org.ssoGroups.claim", "Groups claim")} disabled={!canManage} value={claim} onChange={(e) => setClaim(e.target.value)} />
          }
        />
        <SettingsRow
          title={t("org.ssoGroups.scopes", "Extra scopes")}
          control={
            <Input aria-label={t("org.ssoGroups.scopes", "Extra scopes")} disabled={!canManage} value={scopes} onChange={(e) => setScopes(e.target.value)} />
          }
        />
        <SettingsRow
          title={t("org.ssoGroups.mode", "Mode")}
          description={
            mode === "sync"
              ? t("org.ssoGroups.syncHint", "Takes away what a rule gave once no rule gives it.")
              : t("org.ssoGroups.addHint", "Never takes anything away.")
          }
          control={
            <Segmented
              aria-label={t("org.ssoGroups.mode", "Mode")}
              disabled={!canManage}
              value={mode}
              onValueChange={setMode}
              options={[
                { value: "add", label: t("org.ssoGroups.addOnly", "Add only") },
                { value: "sync", label: t("org.ssoGroups.sync", "Keep in sync") },
              ]}
            />
          }
        />
        <SettingsRow
          title={t("org.ssoGroups.yours", "Groups in your last sign-in")}
          description={initial.your_last_groups?.join(", ") || t("org.ssoGroups.yoursNone", "None yet; sign in through the provider to see yours.")}
        />
        {initial.claim_missing_at && (
          <SettingsRow
            title={t("org.ssoGroups.missing", "Claim missing")}
            description={t("org.ssoGroups.missingHint", "A sign-in {when} lacked the claim; nothing changed.", {
              when: formatRelative(initial.claim_missing_at),
            })}
          />
        )}
      </SettingsGroup>
      <div className="mt-16 flex flex-col gap-16">
        {rules.map((rule, i) => (
          <div key={i} className="flex flex-col gap-8">
            <Text tone="secondary">{t("org.ssoGroups.ruleN", "Rule {n}", { n: i + 1 })}</Text>
            <div className="flex flex-wrap gap-8">
              <Input
                aria-label={t("org.ssoGroups.group", "Group of rule {n}", { n: i + 1 })}
                disabled={!canManage}
                value={rule.group}
                placeholder={t("org.ssoGroups.groupExample", "kaeso-admins")}
                onChange={(e) => edit(i, { ...rule, group: e.target.value })}
              />
              <Select
                aria-label={t("org.ssoGroups.role", "Role of rule {n}", { n: i + 1 })}
                disabled={!canManage}
                value={rule.role ?? NO_ROLE}
                onValueChange={(v) => edit(i, { ...rule, role: v === NO_ROLE ? null : v })}
                options={[
                  { value: NO_ROLE, label: t("org.ssoGroups.noRole", "No organization role") },
                  ...orgRoles.map((r) => ({ value: r.id, label: r.name })),
                ]}
              />
              <Button size="sm" variant="secondary" disabled={!canManage} onClick={() => setRules(rules.filter((_, j) => j !== i))}>
                {t("org.ssoGroups.removeRule", "Remove rule")}
              </Button>
            </div>
            {rule.projects.map((p, k) => (
              <div key={k} className="flex flex-wrap gap-8">
                <Select
                  aria-label={t("org.ssoGroups.project", "Project {k} of rule {n}", { k: k + 1, n: i + 1 })}
                  disabled={!canManage}
                  value={p.project}
                  onValueChange={(v) => editProject(i, rule, k, { project: v })}
                  options={projects.map((x) => ({ value: x.id, label: x.name }))}
                />
                <Select
                  aria-label={t("org.ssoGroups.projectRole", "Role in project {k} of rule {n}", { k: k + 1, n: i + 1 })}
                  disabled={!canManage}
                  value={p.role}
                  onValueChange={(v) => editProject(i, rule, k, { role: v })}
                  options={projectRoles.map((r) => ({ value: r.id, label: r.name }))}
                />
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={!canManage}
                  onClick={() => edit(i, { ...rule, projects: rule.projects.filter((_, m) => m !== k) })}
                >
                  {t("org.ssoGroups.removeProject", "Remove project")}
                </Button>
              </div>
            ))}
            <div>
              <Button
                size="sm"
                variant="secondary"
                disabled={!canManage || !firstProject}
                onClick={() => firstProject && edit(i, { ...rule, projects: [...rule.projects, { project: firstProject.id, role: "developer" }] })}
              >
                {t("org.ssoGroups.addProject", "Add project")}
              </Button>
            </div>
          </div>
        ))}
        <div className="flex gap-8">
          <Button
            variant="secondary"
            disabled={!canManage}
            {...(reason ? { disabledReason: reason } : {})}
            onClick={() => setRules([...rules, { group: "", role: null, projects: [] }])}
          >
            {t("org.ssoGroups.addRule", "Add rule")}
          </Button>
          <Button disabled={!canManage} {...(reason ? { disabledReason: reason } : {})} loading={save.isPending} onClick={() => save.mutate()}>
            {t("org.ssoGroups.save", "Save group rules")}
          </Button>
        </div>
      </div>
    </Section>
  );
}
