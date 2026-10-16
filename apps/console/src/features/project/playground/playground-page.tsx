import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Button, Empty, ModelLabel, Select, icons, type SelectOption } from "@logicplanes/kaeso-ui";
import { CompareGrid, Composer, ParameterPopover, PromptSuggestions, ResponseMetrics, type ReasoningEffort } from "@logicplanes/kaeso-ui/playground";
import { playgroundModelsQuery, type PlaygroundModel } from "~/api/queries/playground";
import { useProjectRights } from "~/auth/rights";
import { providerLabel } from "~/format/labels";
import { usePageCreateAction } from "~/shell/create-action";
import { ProjectHeader } from "~/shell/project-header";
import { metricsOf } from "./metrics";
import { modelRef, requestFor } from "./request";
import { EFFORTS, MAX_COLUMNS, parametersOf, parseModels, searchOf, validatePlaygroundSearch, type PlaygroundSearch } from "./search";
import Thread from "./thread";
import type { Turn } from "./responses";
import { useThreads } from "./use-threads";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { readStored, writeStored } from "~/storage";

const suggestions = () => [
  t("playground.suggestion.raceCondition", "Explain a race condition to a new engineer"),
  t("playground.suggestion.haiku", "Write a haiku about latency"),
  t("playground.suggestion.restGrpc", "Compare REST and gRPC in three bullets"),
];
const fullReason = () => t("playground.full", "Compare up to {count} models at once", { count: MAX_COLUMNS });

const draftKey = (project: string) => `kaeso:playground-draft:${project}`;
function readDraft(project: string): string {
  return readStored(draftKey(project)) ?? "";
}
function writeDraft(project: string, text: string) {
  writeStored(draftKey(project), text || null);
}

const same = (a: object, b: object) => JSON.stringify(a) === JSON.stringify(b);

export function PlaygroundPage({ org, project, search: raw }: { org: string; project: string; search: Record<string, unknown> }) {
  const search = useMemo(() => validatePlaygroundSearch(raw), [raw]);
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const catalogue = useQuery(playgroundModelsQuery(org, project));
  const chat = useMemo(
    () =>
      [...(catalogue.data ?? [])].sort(
        (a, b) => providerLabel(a.provider).localeCompare(providerLabel(b.provider)) || a.display_name.localeCompare(b.display_name),
      ),
    [catalogue.data],
  );
  const byRef = useMemo(() => new Map(chat.map((m) => [modelRef(m), m])), [chat]);

  const callable = chat.filter((m) => m.provider_key);
  const first = callable[0] ?? chat[0];
  const linked = parseModels(search.models);
  const refs = linked.length ? linked : [first ? modelRef(first) : ""];
  const models = refs.map((r) => byRef.get(r) ?? null);

  const setSearch = useCallback(
    (patch: { [K in keyof PlaygroundSearch]?: PlaygroundSearch[K] | undefined }) =>
      void navigate({
        to: "/orgs/$org/projects/$project/playground",
        params: { org, project },
        search: (prev: Record<string, unknown>) =>
          Object.fromEntries(
            Object.entries({ ...validatePlaygroundSearch(prev), ...patch }).filter(([, v]) => v !== undefined && v !== ""),
          ) as PlaygroundSearch,
        replace: true,
      }),
    [navigate, org, project],
  );
  const setRefs = (next: string[]) => setSearch({ models: next.join(",") });

  const linkedParameters = useMemo(() => parametersOf(search), [search]);
  const [parameters, setParameters] = useState(linkedParameters);
  const [seen, setSeen] = useState(linkedParameters);
  if (!same(seen, linkedParameters)) {
    setSeen(linkedParameters);
    if (!same(searchOf(parameters), searchOf(linkedParameters))) setParameters(linkedParameters);
  }
  useEffect(() => {
    const next = searchOf(parameters);
    if (same(next, searchOf(linkedParameters))) return;
    const timer = setTimeout(
      () => setSearch({ system: undefined, temperature: undefined, top_p: undefined, max_tokens: undefined, effort: undefined, summary: undefined, ...next }),
      300,
    );
    return () => clearTimeout(timer);
  }, [parameters, linkedParameters, setSearch]);

  const { threads, busy, send, retry, stop, add, remove, clear } = useThreads(org, project, refs.length);
  const [prompt, setPrompt] = useState(() => readDraft(project));
  const composer = useRef<HTMLTextAreaElement | null>(null);
  const changePrompt = (text: string) => {
    setPrompt(text);
    writeDraft(project, text);
  };

  const full = refs.length >= MAX_COLUMNS;
  const addColumn = () => {
    const m = callable.find((x) => !refs.includes(modelRef(x))) ?? first;
    if (full || !m) return;
    add();
    setRefs([...refs, modelRef(m)]);
  };
  const removeColumn = (i: number) => {
    remove(i);
    setRefs(refs.filter((_, j) => j !== i));
  };
  const submit = (text: string) => {
    const sends = threads.flatMap((thread, i) => {
      const m = models[i];
      return m ? [{ column: i, request: requestFor(m, thread.turns, text, parameters), label: m.display_name }] : [];
    });
    send(text, sends);
    changePrompt("");
  };
  const retryColumn = (i: number, m: PlaygroundModel, earlier: Turn[], last: Turn) => retry(i, requestFor(m, earlier, last.prompt, parameters), m.display_name);

  const noModels = catalogue.isSuccess && chat.length === 0;
  usePageCreateAction(noModels ? null : { label: t("playground.addColumn", "Add column"), run: addColumn, disabledReason: full ? fullReason() : undefined });

  const empty = threads.every((x) => x.turns.length === 0);
  const options = useMemo(() => chat.map(modelOption), [chat]);
  const chosen = models.filter((m): m is PlaygroundModel => !!m);
  const reasoners = chosen.filter((m) => m.reasoning);
  const efforts: ReasoningEffort[] | boolean = reasoners.length
    ? reasoners.some((m) => m.efforts.length === 0)
      ? true
      : EFFORTS.filter((e) => reasoners.some((m) => m.efforts.includes(e)))
    : false;

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("playground.title", "Playground")}
        path="playground"
        actions={
          <div className="flex items-center gap-8">
            <Button
              size="sm"
              variant="ghost"
              disabled={empty}
              disabledReason={empty ? t("playground.nothingToClear", "Nothing to clear yet") : undefined}
              onClick={clear}
            >
              {common.clear()}
            </Button>
            <ParameterPopover
              value={parameters}
              onValueChange={setParameters}
              sampling={chosen.length === 0 || chosen.some((m) => m.temperature)}
              reasoning={efforts}
              trigger={
                <Button size="sm" disabled={noModels}>
                  {t("playground.parameters", "Parameters")}
                </Button>
              }
            />
            <Button size="sm" leadingIcon={icons.add} disabled={full || noModels} disabledReason={full ? fullReason() : undefined} onClick={addColumn}>
              {t("playground.addColumn", "Add column")}
            </Button>
          </div>
        }
      />
      <AppFrame.Body padding="none">
        {noModels ? (
          <Empty
            size="lg"
            title={t("playground.empty", "No models to try yet")}
            description={t("playground.emptyHint", "Models in this project's catalogue that you may call appear here.")}
            actions={
              rights.has("models.manage") ? (
                <Button variant="primary" href={`/orgs/${org}/projects/${project}/models?add=true`}>
                  {t("playground.addModels", "Add models")}
                </Button>
              ) : undefined
            }
          />
        ) : (
          <div className="flex h-full min-h-0 flex-col">
            <div className="min-h-0 flex-1">
              <CompareGrid aria-label={common.models()}>
                {refs.map((ref, i) => {
                  const m = models[i] ?? null;
                  const thread = threads[i];
                  const lastTurn = thread?.turns.at(-1);
                  const last = lastTurn?.response ?? null;
                  const label = m?.display_name ?? common.model();
                  return (
                    <CompareGrid.Column
                      key={thread?.key ?? `col${i}`}
                      label={label}
                      header={
                        <Select
                          aria-label={common.model()}
                          appearance="ghost"
                          size="sm"
                          placeholder={catalogue.isPending ? t("playground.loadingModels", "Loading models") : t("playground.chooseModel", "Choose a model")}
                          value={m ? ref : null}
                          onValueChange={(v) => setRefs(refs.map((r, j) => (j === i ? v : r)))}
                          options={options}
                          disabled={catalogue.isPending}
                          fullWidth
                        />
                      }
                      onRemove={() => removeColumn(i)}
                      removeLabel={t("playground.removeColumn", "Remove {model}", { model: label })}
                      footer={<ResponseMetrics {...metricsOf(last, m)} />}
                    >
                      {thread && lastTurn ? (
                        <Thread
                          turns={thread.turns}
                          model={m}
                          label={label}
                          onRetry={m && !busy ? () => retryColumn(i, m, thread.turns.slice(0, -1), lastTurn) : null}
                        />
                      ) : null}
                    </CompareGrid.Column>
                  );
                })}
              </CompareGrid>
            </div>
            <div className="mx-16 mt-12 mb-16 flex flex-col gap-8">
              {empty && (
                <PromptSuggestions
                  suggestions={suggestions()}
                  onSelect={(p) => {
                    changePrompt(p);
                    composer.current?.focus();
                  }}
                />
              )}
              <Composer
                ref={composer}
                value={prompt}
                onValueChange={changePrompt}
                onSubmit={submit}
                onStop={stop}
                streaming={busy}
                disabled={catalogue.isPending || chosen.length === 0}
                placeholder={refs.length > 1 ? t("playground.promptEvery", "Send a prompt to every model") : t("playground.prompt", "Send a prompt")}
              />
            </div>
          </div>
        )}
      </AppFrame.Body>
    </>
  );
}

function modelOption(m: PlaygroundModel): SelectOption {
  const name = m.display_name;
  return {
    value: modelRef(m),
    textValue: name,
    group: providerLabel(m.provider),
    label: <ModelLabel provider={m.provider} model={m.model_id} {...(name !== m.model_id ? { name } : {})} capabilities={m.reasoning ? ["reasoning"] : []} />,
    ...(m.provider_key
      ? {}
      : { disabled: true, description: t("playground.noProviderKey", "No {provider} key for this project", { provider: providerLabel(m.provider) }) }),
  };
}
