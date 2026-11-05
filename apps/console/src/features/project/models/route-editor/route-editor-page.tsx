import { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useBlocker, useNavigate } from "@tanstack/react-router";
import { AlertDialog, AppFrame, Button, DialogScope, IconButton, Loader, Menu, PageHeader, Tabs, icons } from "@logicplanes/kaeso-ui";
import { FlowCanvas, FlowWorkspace } from "@logicplanes/kaeso-ui/flow-canvas";
import { formatDuration } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { AliasView, CatalogueModelView, RouteStep } from "~/api/generated/types";
import { allKeysQuery } from "~/api/queries/keys";
import { projectMembersQuery, serviceAccountsQuery } from "~/api/queries/members";
import { aliasQuery, catalogueQuery } from "~/api/queries/models";
import { projectCatalogue } from "~/api/queries/invalidation";
import { useApiMutation } from "~/auth/mutation";
import { useRequestLogs, useSession } from "~/auth/session";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { LoadFailed } from "~/features/org/load-failed";
import { catalogueModelOption } from "~/kaeso/model-options";
import { useModelPickerProviders, useProviders } from "~/kaeso/providers";
import { toast } from "~/shell/toast";
import { usePageTitle } from "~/shell/title";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { DeleteAliasDialog } from "../delete-alias-dialog";
import {
  add,
  defaultRemoval,
  edgeId,
  emptyModel,
  find,
  fromRoute,
  map,
  move,
  moves,
  parentEdge,
  parseEdge,
  problems as findProblems,
  remove,
  removals,
  sameRoute,
  stepAt,
  swapBranches,
  template,
  toRoute,
  walk,
  type DraftStep,
  type Removal,
  type StepKind,
  type Template,
} from "./draft";
import { useHistory } from "./history";
import { AliasPanel, ConditionPanel, ModelPanel, SplitPanel, type FieldErrors, type InspectorContext } from "./inspector";
import { moveLabel, type Names } from "./labels";
import { RouteJsonDialog } from "./route-json";
import { pathIds, toneOf, useRouteSend, type Walked } from "./send";
import { TestPanel, asSent, firstRequest, sentHeaders, useTrace, type TestMode, type TestRequest } from "./test-panel";
import { START, canvasTree, stepName, type Outcome } from "./tree";
import { AliasAuditLog, type Version } from "./audit";

type Doc = { name: string; root: DraftStep };
type Tab = "editor" | "test" | "audit";

const KIND_ICON = { model: icons.model, condition: icons.condition, split: icons.split } as const;
const KINDS = ["model", "condition", "split"] as const;
type Saved = { id: string; name: string; route: RouteStep };

function outcome(w: Walked): Outcome {
  if (w.status === null) return { tone: "neutral", label: t("models.route.sent.skipped", "Skipped") };
  if (w.ms === null) return { tone: toneOf(w.status), label: String(w.status) };
  return {
    tone: toneOf(w.status),
    label: t("models.route.sent.statusAfter", "{status} after {duration}", { status: String(w.status), duration: formatDuration(w.ms) }),
  };
}

function outcomes(root: DraftStep, walked: readonly Walked[]): Map<string, Outcome> {
  const byPath = new Map(walked.map((w) => [w.path, w]));
  const out = new Map<string, Outcome>();
  for (const { step, path } of walk(root)) {
    const w = byPath.get(path);
    if (w) out.set(step.id, outcome(w));
  }
  return out;
}

export function RouteEditorPage({ org, project, alias }: { org: string; project: string; alias?: string | undefined }) {
  const catalogue = useQuery(catalogueQuery(org, project));
  const one = useQuery({ ...aliasQuery(org, project, alias ?? ""), enabled: !!alias });
  const title = alias ? (one.data?.name ?? t("models.aliases", "Aliases")) : t("models.route.newAlias", "New alias");
  usePageTitle(title, common.models());
  if (catalogue.isError || one.isError)
    return (
      <Frame org={org} project={project} title={title}>
        <LoadFailed
          title={alias ? t("models.route.loadFailed", "This alias couldn't load") : t("models.route.catalogueFailed", "The catalogue couldn't load")}
        />
      </Frame>
    );
  if (catalogue.isPending || (alias && one.isPending))
    return (
      <Frame org={org} project={project} title={title}>
        <div className="flex flex-1 items-center justify-center">
          <Loader />
        </div>
      </Frame>
    );
  const saved = one.data ? { id: one.data.id, name: one.data.name, route: one.data.route } : null;
  return <Editor key={saved?.id ?? "new"} org={org} project={project} catalogue={catalogue.data} initial={saved} view={one.data} />;
}

function Frame({ org, project, title, children }: { org: string; project: string; title: string; children: React.ReactNode }) {
  return (
    <>
      <PageHeader start={<Start org={org} project={project} />} title={title} />
      {children}
    </>
  );
}

function Start({ org, project }: { org: string; project: string }) {
  const navigate = useNavigate();
  return (
    <span className="flex items-center gap-4">
      <AppFrame.MenuButton />
      <IconButton
        icon={icons.chevronLeft}
        size="sm"
        variant="ghost"
        aria-label={t("models.route.back", "Back to aliases")}
        onClick={() => void navigate({ to: "/orgs/$org/projects/$project/models", params: { org, project }, search: { tab: "aliases" } })}
      />
    </span>
  );
}

const nameMissing = (name: string) => name.trim() === "";

const isTyping = (e: KeyboardEvent) => {
  const el = e.target as HTMLElement | null;
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);
};

function Editor({
  org,
  project,
  catalogue,
  initial,
  view,
}: {
  org: string;
  project: string;
  catalogue: CatalogueModelView[];
  initial: Saved | null;
  view: AliasView | undefined;
}) {
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const needs = useNeedsRight(org, project);
  const readOnly = !rights.has("models.manage");
  const canSeeAudit = rights.has("project.audit_log.view");
  const history = useHistory<Doc>(() => ({ name: initial?.name ?? "", root: initial ? fromRoute(initial.route) : emptyModel() }));
  const doc = history.present;
  const [saved, setSaved] = useState<Saved | null>(initial);
  const route = useMemo(() => toRoute(doc.root), [doc.root]);
  const dirty = !saved || saved.name !== doc.name.trim() || !sameRoute(saved.route, route);
  const [selected, setSelected] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("editor");
  const [request, setRequest] = useState<TestRequest>(() => firstRequest(doc.root));
  const [mode, setMode] = useState<TestMode>("send");
  const [prompt, setPrompt] = useState(() => t("models.route.defaultPrompt", "Say hello in one sentence."));
  const me = useSession().user.id;
  const logs = useRequestLogs();
  const sender = useRouteSend(org, project);
  const [started, setStarted] = useState<{ kind: Template; root: DraftStep } | null>(() => (initial ? null : { kind: "single", root: history.present.root }));
  const [json, setJson] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [nameError, setNameError] = useState<string>();
  const [stepErrors, setStepErrors] = useState<FieldErrors>(new Map());
  const leaving = useRef(false);

  const models = useMemo(() => new Map(catalogue.map((m) => [m.id, m])), [catalogue]);
  const known = useCallback((id: string) => models.has(id), [models]);
  const problems = useMemo(() => findProblems(doc.root, known), [doc.root, known]);
  const shadows = useMemo(() => catalogue.some((m) => m.model_id === doc.name.trim()), [catalogue, doc.name]);
  const providerIds = useMemo(() => [...new Set(catalogue.map((m) => m.provider))], [catalogue]);
  const pickerProviders = useModelPickerProviders(org, providerIds);
  const providers = useProviders(org);
  const options = useMemo(() => catalogue.map(catalogueModelOption), [catalogue]);

  const usesWho = useMemo(() => {
    const fields = new Set<string>();
    for (const { step } of walk(doc.root)) if (step.type === "condition") for (const r of step.rules) fields.add(r.field);
    return fields;
  }, [doc.root]);
  const keys = useQuery({ ...allKeysQuery(org, project), enabled: usesWho.has("key") || tab === "test" });
  const members = useQuery({ ...projectMembersQuery(org, project), enabled: usesWho.has("member") });
  const accounts = useQuery({ ...serviceAccountsQuery(org, project), enabled: usesWho.has("service_account") });
  const names: Names = useMemo(
    () => ({
      key: (id) => keys.data?.find((k) => k.id === id)?.name ?? id,
      member: (id) => members.data?.find((m) => m.user.id === id)?.user.name ?? id,
      serviceAccount: (id) => accounts.data?.find((a) => a.id === id)?.name ?? id,
    }),
    [keys.data, members.data, accounts.data],
  );

  const testing = tab === "test";
  const testable = problems.size === 0;
  const sending = testing && mode === "send";
  const readsTokens = useMemo(
    () => [...walk(doc.root)].some(({ step }) => step.type === "condition" && step.rules.some((r) => r.field === "input_tokens")),
    [doc.root],
  );
  const typed = useDeferredValue(readsTokens ? prompt : "");
  const test = useTrace({
    org,
    project,
    alias: saved?.id,
    route,
    root: doc.root,
    request: sending ? asSent(request, typed, me) : request,
    keys: keys.data,
    enabled: testing && testable,
  });
  const trace = testing && testable ? test.trace : undefined;
  const result = sending && sender.sent?.walked && sameRoute(sender.sent.route, route) ? sender.sent.walked : null;
  const sendBlock = !rights.has("playground.use")
    ? needs("playground.use")
    : nameMissing(doc.name)
      ? t("models.route.needsName", "Give the alias a name first.")
      : undefined;
  const sendRequest = (text: string) => {
    void sender.send(text, {
      model: doc.name.trim(),
      input: text,
      stream: true,
      route,
      ...(saved ? { alias: saved.id } : {}),
      headers: sentHeaders(request),
    });
  };

  const tree = useMemo(
    () =>
      canvasTree({
        alias: doc.name,
        root: doc.root,
        models,
        providerName: providers.name,
        problems,
        names,
        ...(result ? { outcomes: outcomes(doc.root, result) } : { tries: trace?.tries }),
      }),
    [doc.name, doc.root, models, providers.name, problems, names, trace, result],
  );

  const clearErrors = () => {
    setNameError(undefined);
    setStepErrors(new Map());
  };
  const change: InspectorContext["onChange"] = (id, fn, coalesce) => {
    clearErrors();
    history.commit((d) => ({ ...d, root: map(d.root, id, fn) }), coalesce);
  };
  const addStep = (edgeId: string, kind: StepKind, insert: boolean) => {
    const edge = parseEdge(edgeId);
    clearErrors();
    const next = add(doc.root, edge, kind, insert);
    history.commit((d) => ({ ...d, root: next.root }));
    setSelected(next.added);
  };
  const removeStep = (id: string, how: Removal) => {
    clearErrors();
    const next = remove(doc.root, id, how);
    history.commit((d) => ({ ...d, root: next }));
    setSelected((s) => (s && find(next, s) ? s : null));
  };
  const addFallback = (id: string) => addStep(`fallback:${id}`, "model", false);
  const reshape = (next: DraftStep) => {
    clearErrors();
    history.commit((d) => ({ ...d, root: next }));
  };
  const openStep = (id: string | null) => {
    setTab("editor");
    setSelected(id === START ? null : id);
  };

  const save = useApiMutation({
    mutationFn: async () => {
      const name = doc.name.trim();
      if (saved)
        return data(
          await api.PATCH("/platform/v1/orgs/{org}/projects/{project}/aliases/{alias}", {
            params: { path: { org, project, alias: saved.id } },
            body: { ...(name !== saved.name ? { name } : {}), ...(!sameRoute(saved.route, route) ? { route } : {}) },
          }),
        );
      return data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/aliases", { params: { path: { org, project } }, body: { name, route } }));
    },
    invalidates: projectCatalogue(org, project),
    onSuccess: (a) => {
      const created = !saved;
      setSaved({ id: a.id, name: a.name, route: a.route });
      toast(created ? t("models.alias.created", "Alias {name} created", { name: a.name }) : t("models.alias.saved", "Alias {name} saved", { name: a.name }));
      if (created) {
        leaving.current = true;
        void navigate({ to: "/orgs/$org/projects/$project/models/aliases/$alias", params: { org, project, alias: a.id }, replace: true });
      }
    },
    onError: (e) => {
      if (e.param === "name") setNameError(e.message);
      if (e.param?.startsWith("route")) {
        const at = stepAt(doc.root, e.param);
        setStepErrors(new Map([[at.step.id, { field: at.field, message: e.message }]]));
        setSelected(at.step.id);
        setTab("editor");
      }
    },
  });

  const blockingProblems = problems.size;
  const saveReason = readOnly
    ? needs("models.manage")
    : nameMissing(doc.name)
      ? t("models.route.needsName", "Give the alias a name first.")
      : blockingProblems > 0
        ? t("models.route.needsFixes", "Fix the steps marked on the route first.")
        : !dirty
          ? t("models.route.nothingToSave", "Nothing has changed since the last save.")
          : undefined;
  const canSave = !saveReason;

  const blocker = useBlocker({ shouldBlockFn: () => dirty && !leaving.current && !readOnly, enableBeforeUnload: () => dirty && !readOnly, withResolver: true });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;
      const key = e.key.toLowerCase();
      if (key === "s") {
        e.preventDefault();
        if (canSave && !save.isPending) save.mutate();
        return;
      }
      if (isTyping(e) || readOnly || tab !== "editor") return;
      if (key === "z" && !e.shiftKey) {
        e.preventDefault();
        history.undo();
      } else if ((key === "z" && e.shiftKey) || key === "y") {
        e.preventDefault();
        history.redo();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [canSave, save, readOnly, tab, history]);

  const selectedStep = selected && selected !== START ? find(doc.root, selected) : undefined;
  const ctx: InspectorContext = {
    org,
    project,
    root: doc.root,
    readOnly,
    options,
    providers: pickerProviders,
    problems,
    errors: stepErrors,
    stepName: (s) => stepName(s, { models, names }),
    onChange: change,
    onRemove: removeStep,
    onAddFallback: addFallback,
  };

  const kindLabel = (kind: StepKind, insert: boolean) =>
    insert
      ? kind === "model"
        ? t("models.route.insertModel", "Try a model first")
        : kind === "condition"
          ? t("models.route.insertCondition", "Branch on a condition")
          : t("models.route.insertSplit", "Split the traffic")
      : kind === "model"
        ? t("models.route.addModel", "Fall back to a model")
        : kind === "condition"
          ? t("models.route.addCondition", "Fall back by a condition")
          : t("models.route.addSplit", "Fall back to a split");

  const addMenu = (slot: { edge: string; insert: boolean }) => (
    <>
      {KINDS.map((kind) => (
        <Menu.Item key={kind} icon={KIND_ICON[kind]} onClick={() => addStep(slot.edge, kind, slot.insert)}>
          {kindLabel(kind, slot.insert)}
        </Menu.Item>
      ))}
    </>
  );

  const nodeActions = (id: string) => {
    if (id === START) return null;
    const step = find(doc.root, id);
    const parent = parentEdge(doc.root, id);
    const ways = removals(doc.root, id);
    const shifts = moves(doc.root, id);
    return (
      <>
        {parent &&
          KINDS.map((kind) => (
            <Menu.Item key={kind} icon={KIND_ICON[kind]} onClick={() => addStep(edgeId(parent), kind, true)}>
              {kindLabel(kind, true)}
            </Menu.Item>
          ))}
        {(shifts.length > 0 || step?.type === "condition") && <Menu.Separator />}
        {shifts.map((to) => (
          <Menu.Item key={to} icon={to === "earlier" ? icons.moveUp : icons.moveDown} onClick={() => reshape(move(doc.root, id, to))}>
            {moveLabel[to]()}
          </Menu.Item>
        ))}
        {step?.type === "condition" && (
          <Menu.Item icon={icons.swap} onClick={() => reshape(swapBranches(doc.root, id))}>
            {t("models.route.swapBranches", "Swap Then and Else")}
          </Menu.Item>
        )}
        {ways.length > 0 && (
          <>
            <Menu.Separator />
            <Menu.Item icon={icons.delete} variant="danger" onClick={() => ways[0] && removeStep(id, ways[0])}>
              {t("models.route.removeStep", "Remove step")}
            </Menu.Item>
          </>
        )}
      </>
    );
  };

  const restore = (v: Version) => {
    clearErrors();
    history.commit(() => ({ name: v.name, root: fromRoute(v.route) }));
    setTab("editor");
    setSelected(null);
    toast(t("models.route.restored", "Version restored; save to make it live"));
  };

  const panel = testing ? (
    <TestPanel
      mode={mode}
      onMode={setMode}
      request={request}
      onRequest={setRequest}
      prompt={prompt}
      onPrompt={setPrompt}
      root={doc.root}
      blocked={!testable}
      sendBlock={sendBlock}
      trace={trace}
      error={test.error}
      loading={test.loading}
      sent={sender.sent}
      busy={sender.busy}
      onSend={sendRequest}
      onStop={sender.stop}
      onOpenLog={
        logs && rights.has("logs.metadata.view")
          ? (log: string) => void navigate({ to: "/orgs/$org/projects/$project/logs", params: { org, project }, search: { log } })
          : undefined
      }
      keys={keys.data}
      models={models}
      names={names}
      providerName={providers.name}
      stepName={(s) => stepName(s, { models, names })}
    />
  ) : selectedStep?.type === "model" ? (
    <ModelPanel key={selectedStep.id} ctx={ctx} step={selectedStep} />
  ) : selectedStep?.type === "condition" ? (
    <ConditionPanel key={selectedStep.id} ctx={ctx} step={selectedStep} />
  ) : selectedStep?.type === "split" ? (
    <SplitPanel key={selectedStep.id} ctx={ctx} step={selectedStep} />
  ) : (
    <AliasPanel
      name={doc.name}
      onName={(name) => {
        setNameError(undefined);
        history.commit((d) => ({ ...d, name }), "name");
      }}
      nameError={nameError}
      readOnly={readOnly}
      shadows={shadows}
      problems={blockingProblems}
      {...(!saved && !readOnly
        ? {
            templates: {
              value: started && started.root === doc.root ? started.kind : null,
              onPick: (kind: Template) => {
                const next = template(
                  kind,
                  catalogue.map((m) => m.id),
                );
                reshape(next);
                setStarted({ kind, root: next });
                setRequest(firstRequest(next));
              },
            },
          }
        : {})}
    />
  );

  return (
    <>
      <PageHeader
        start={<Start org={org} project={project} />}
        title={saved?.name ?? (doc.name.trim() || t("models.route.newAlias", "New alias"))}
        tabs={
          <Tabs variant="pill" value={tab} onValueChange={(v) => setTab(v === "audit" || v === "test" ? v : "editor")}>
            <Tabs.List aria-label={t("models.route.views", "Alias views")}>
              <Tabs.Tab value="editor">{t("models.route.editor", "Editor")}</Tabs.Tab>
              <Tabs.Tab value="test">{t("models.route.testTab", "Test")}</Tabs.Tab>
              {saved && canSeeAudit && <Tabs.Tab value="audit">{t("models.route.auditLog", "Audit log")}</Tabs.Tab>}
            </Tabs.List>
          </Tabs>
        }
        actions={
          <span className="flex items-center gap-8">
            {!readOnly && (
              <>
                <IconButton
                  icon={icons.undo}
                  variant="ghost"
                  aria-label={t("models.route.undo", "Undo")}
                  disabled={!history.canUndo || tab !== "editor"}
                  onClick={history.undo}
                />
                <IconButton
                  icon={icons.redo}
                  variant="ghost"
                  aria-label={t("models.route.redo", "Redo")}
                  disabled={!history.canRedo || tab !== "editor"}
                  onClick={history.redo}
                />
              </>
            )}
            <IconButton icon={icons.code} variant="ghost" aria-label={t("models.route.showJson", "Show as JSON")} onClick={() => setJson(true)} />
            {saved && !readOnly && (
              <IconButton icon={icons.delete} variant="ghost" aria-label={t("models.alias.delete", "Delete alias")} onClick={() => setDeleting(true)} />
            )}
            {!readOnly && (
              <Button
                variant="primary"
                loading={save.isPending}
                disabled={!canSave}
                {...(saveReason ? { disabledReason: saveReason } : {})}
                onClick={() => canSave && save.mutate()}
              >
                {saved ? t("models.route.save", "Save") : t("models.createAlias", "Create alias")}
              </Button>
            )}
          </span>
        }
      />
      {tab === "audit" && saved ? (
        <AliasAuditLog org={org} project={project} alias={saved.id} current={{ name: doc.name, route }} readOnly={readOnly} onRestore={restore} />
      ) : (
        <FlowWorkspace panel={panel} panelLabel={t("models.route.inspector", "Step settings")}>
          <FlowCanvas
            aria-label={t("models.route.canvasLabel", "Route of {name}", { name: doc.name.trim() || t("models.route.newAlias", "New alias") })}
            root={tree}
            selected={testing ? null : (selected ?? null)}
            trace={result ? [START, ...pathIds(doc.root, result)] : trace ? [START, ...trace.ids] : undefined}
            busy={sending && sender.busy && !result}
            onSelect={(id) => {
              if (testing) {
                if (id) openStep(id);
                return;
              }
              setSelected(id);
            }}
            {...(!readOnly && !testing
              ? {
                  addMenu,
                  nodeActions,
                  onDelete: (id: string) => {
                    const how = defaultRemoval(doc.root, id);
                    if (how) removeStep(id, how);
                  },
                }
              : {})}
          />
        </FlowWorkspace>
      )}
      <RouteJsonDialog open={json} onOpenChange={setJson} name={doc.name.trim()} route={route} />
      {saved && view && (
        <DialogScope open={deleting}>
          <DeleteAliasDialog
            org={org}
            project={project}
            alias={{ ...view, id: saved.id, name: saved.name }}
            open={deleting}
            onOpenChange={setDeleting}
            onDeleted={() => {
              leaving.current = true;
              void navigate({ to: "/orgs/$org/projects/$project/models", params: { org, project }, search: { tab: "aliases" } });
            }}
          />
        </DialogScope>
      )}
      <AlertDialog
        open={blocker.status === "blocked"}
        onOpenChange={(o) => !o && blocker.reset?.()}
        tone="danger"
        title={t("models.route.leaveTitle", "Leave without saving?")}
        description={t("models.route.leaveHint", "Your changes to this route are lost; requests keep taking the saved route.")}
        confirmLabel={t("models.route.leave", "Leave")}
        onConfirm={() => blocker.proceed?.()}
      />
    </>
  );
}
