import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button, Checkbox, Dialog, Empty, List, Section, Text, icons } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { GuardrailView, ProjectGuardrailView } from "~/api/generated/types";
import { guardrailsKey, guardrailsQuery, projectGuardrailsKey, projectGuardrailsQuery } from "~/api/queries/guardrails";
import { useApiMutation } from "~/auth/mutation";
import { useNeedsRight } from "~/auth/needs-right";
import { useOrgRights } from "~/auth/session";
import { phaseWords } from "~/features/org/guardrails-page";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { toast } from "~/shell/toast";

export function GuardrailsSection({ org, project }: { org: string; project: string }) {
  const rights = useOrgRights(org);
  const needs = useNeedsRight(org);
  const canManage = rights.has("guardrails.manage");
  const used = useQuery(projectGuardrailsQuery(org, project));
  const [choosing, setChoosing] = useState(false);
  const list = used.data ?? [];
  if (used.isSuccess && list.length === 0 && !canManage) return null;
  return (
    <Section
      spacing="settings"
      title={t("project.guardrails.title", "Guardrails")}
      description={t("project.guardrails.hint", "The organization's checks on this project's chat requests, in order.")}
      actions={
        <Button size="sm" disabled={!canManage} {...(canManage ? {} : { disabledReason: needs("guardrails.manage") })} onClick={() => setChoosing(true)}>
          {t("project.guardrails.choose", "Choose guardrails")}
        </Button>
      }
    >
      {used.isError ? (
        <Text tone="secondary">{t("project.guardrails.loadFailed", "The guardrails couldn't load; the notification says why.")}</Text>
      ) : used.isSuccess && list.length === 0 ? (
        <Text tone="secondary">{t("project.guardrails.none", "No guardrails; requests go to models unchecked.")}</Text>
      ) : (
        <List
          aria-label={t("project.guardrails.title", "Guardrails")}
          loading={used.isPending}
          items={list.map((g: ProjectGuardrailView, i) => ({
            id: g.id,
            textValue: g.name,
            title: g.name,
            subtitle: (
              <Text variant="small" tone="secondary">
                {phaseWords(g)}
              </Text>
            ),
            meta: (
              <Text variant="small" tone="secondary">
                {g.on_error === "allow"
                  ? t("project.guardrails.allowsOnError", "Lets requests through when it fails")
                  : t("project.guardrails.blocksOnError", "Refuses requests when it fails")}
              </Text>
            ),
            date: (
              <Text variant="small" tone="secondary">
                {t("project.guardrails.position", "{n} of {count}", { n: i + 1, count: list.length })}
              </Text>
            ),
          }))}
        />
      )}
      {choosing && <ChooseDialog org={org} project={project} current={list} onClose={() => setChoosing(false)} />}
    </Section>
  );
}

function ChooseDialog({ org, project, current, onClose }: { org: string; project: string; current: ProjectGuardrailView[]; onClose: () => void }) {
  const all = useQuery(guardrailsQuery(org));
  const [chosen, setChosen] = useState<string[]>(current.map((g) => g.id));
  const list = all.data ?? [];
  const live = chosen.filter((id) => list.some((g) => g.id === id));
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PUT("/platform/v1/orgs/{org}/guardrails/projects", {
          params: { path: { org } },
          body: { project, guardrails: live },
        }),
      ),
    invalidates: [projectGuardrailsKey(org, project), guardrailsKey(org)],
    onSuccess: () => {
      toast.success(t("project.guardrails.saved", "Guardrails saved"));
      onClose();
    },
  });
  const rank = (g: GuardrailView) => {
    const i = live.indexOf(g.id);
    return i < 0 ? live.length + list.indexOf(g) : i;
  };
  const ordered = [...list].sort((a, b) => rank(a) - rank(b));
  const toggle = (id: string, on: boolean) => setChosen(on ? [...live, id] : live.filter((x) => x !== id));
  const move = (id: string, by: -1 | 1) => {
    const next = live.filter((x) => x !== id);
    next.splice(live.indexOf(id) + by, 0, id);
    setChosen(next);
  };
  return (
    <Dialog
      open
      busy={save.isPending}
      onOpenChange={(o) => !o && onClose()}
      title={t("project.guardrails.choose", "Choose guardrails")}
      description={t("project.guardrails.chooseHint", "Requests pass the chosen guardrails from top to bottom.")}
      footer={
        <>
          <Dialog.Close>
            <Button>{common.cancel()}</Button>
          </Dialog.Close>
          <Button variant="primary" loading={save.isPending} onClick={() => save.mutate()}>
            {common.save()}
          </Button>
        </>
      }
    >
      {all.isSuccess && ordered.length === 0 ? (
        <Empty
          size="compact"
          title={t("org.guardrails.none", "No guardrails yet")}
          description={t("project.guardrails.addFirst", "Add one under Organization settings › Guardrails first.")}
        />
      ) : (
        <div className="flex flex-col gap-8">
          {ordered.map((g) => {
            const i = live.indexOf(g.id);
            return (
              <div key={g.id} className="flex items-center gap-8">
                <div className="min-w-0 flex-1">
                  <Checkbox checked={i >= 0} label={g.name} description={phaseWords(g)} onCheckedChange={(on) => toggle(g.id, on)} />
                </div>
                {i >= 0 && (
                  <>
                    <Button
                      variant="ghost"
                      size="xs"
                      icon={icons.moveUp}
                      aria-label={t("project.guardrails.up", "Move {name} up", { name: g.name })}
                      disabled={i === 0}
                      onClick={() => move(g.id, -1)}
                    />
                    <Button
                      variant="ghost"
                      size="xs"
                      icon={icons.moveDown}
                      aria-label={t("project.guardrails.down", "Move {name} down", { name: g.name })}
                      disabled={i === live.length - 1}
                      onClick={() => move(g.id, 1)}
                    />
                  </>
                )}
              </div>
            );
          })}
        </div>
      )}
    </Dialog>
  );
}
