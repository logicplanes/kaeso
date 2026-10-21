import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Button, Empty, icons } from "@logicplanes/kaeso-ui";
import {
  CompareGrid,
  Composer,
  ModelPicker,
  ParameterPopover,
  PromptSuggestions,
  ResponseMetrics,
  type ReasoningEffort,
} from "@logicplanes/kaeso-ui/playground";
import { playgroundModelsQuery, type PlaygroundModel } from "~/api/queries/playground";
import { useProjectRights } from "~/auth/rights";
import { playgroundModelOption, recentModels, rememberModel } from "~/kaeso/model-options";
import { useModelPickerProviders, useProviders } from "~/kaeso/providers";
import { usePageCreateAction } from "~/shell/create-action";
import { ProjectHeader } from "~/shell/project-header";
import { metricsOf } from "./metrics";
import { modelRef, requestFor } from "./request";
import { parametersOf, searchOf } from "./search";
import { EFFORTS, MAX_COLUMNS, parseModels, validatePlaygroundSearch, type PlaygroundSearch } from "~/format/playground-search";
import Thread from "./thread";
import type { Turn } from "./responses";
import { useThreads, type Thread as ThreadState } from "./use-threads";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { readStored, writeStored } from "~/storage";
import { PLAYGROUND_CONVERSATION, playgroundDraftKey } from "~/auth/user-data";
import { forget, recall, remember } from "~/shell/session-memory";
import { toast } from "~/shell/toast";
import { patchSearch, type SearchPatch } from "~/search";

const suggestions = () => [
  t("playground.suggestion.raceCondition", "Explain a race condition to a new engineer"),
  t("playground.suggestion.haiku", "Write a haiku about latency"),
  t("playground.suggestion.restGrpc", "Compare REST and gRPC in three bullets"),
];
const fullReason = () => t("playground.full", "Compare up to {count} models at once", { count: MAX_COLUMNS });

function readDraft(project: string): string {
  return readStored(playgroundDraftKey(project)) ?? "";
}
function writeDraft(project: string, text: string) {
  writeStored(playgroundDraftKey(project), text || null);
}

const same = (a: object, b: object) => JSON.stringify(a) === JSON.stringify(b);

interface Conversation {
  refs: string[];
  threads: ThreadState[];
}
const conversationKey = (project: string) => [...PLAYGROUND_CONVERSATION, project];
const warnOnLeave = (e: BeforeUnloadEvent) => e.preventDefault();
const CLEARED = "playground:cleared";

export function PlaygroundPage({ org, project, search }: { org: string; project: string; search: PlaygroundSearch }) {
  const navigate = useNavigate();
  const rights = useProjectRights(org, project);
  const catalogue = useQuery(playgroundModelsQuery(org, project));
  const providers = useProviders(org);
  const chat = useMemo(
    () => [...(catalogue.data ?? [])].sort((a, b) => providers.byName(a.provider, b.provider) || a.display_name.localeCompare(b.display_name)),
    [catalogue.data, providers],
  );
  const byRef = useMemo(() => new Map(chat.map((m) => [modelRef(m), m])), [chat]);

  const callable = chat.filter((m) => m.provider_key && m.availability !== "unavailable");
  const first = callable[0] ?? chat[0];
  const [saved] = useState(() => recall<Conversation>(conversationKey(project)));
  const linked = parseModels(search.models);
  const refs = linked.length ? linked : (saved?.refs ?? [first ? modelRef(first) : ""]);
  const models = refs.map((r) => byRef.get(r) ?? null);

  const setSearch = useCallback(
    (patch: SearchPatch<PlaygroundSearch>) =>
      void navigate({
        to: "/orgs/$org/projects/$project/playground",
        params: { org, project },
        search: (prev: Record<string, unknown>) => patchSearch(validatePlaygroundSearch(prev), patch),
        replace: true,
      }),
    [navigate, org, project],
  );
  const fallback = first ? modelRef(first) : "";
  const updateRefs = useCallback(
    (next: (current: string[]) => string[]) =>
      void navigate({
        to: "/orgs/$org/projects/$project/playground",
        params: { org, project },
        search: (prev: Record<string, unknown>) => {
          const current = validatePlaygroundSearch(prev);
          const linkedNow = parseModels(current.models);
          return patchSearch(current, { models: next(linkedNow.length ? linkedNow : [fallback]).join(",") });
        },
        replace: true,
      }),
    [navigate, org, project, fallback],
  );

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

  const shownRefs = useRef(refs);
  useEffect(() => {
    shownRefs.current = refs;
  });
  const { threads, busy, send, retry, stop, add, remove, clear, restore } = useThreads(org, project, refs.length, {
    initial: saved && same(saved.refs, refs) ? saved.threads : undefined,
    onLeave: (left) => {
      if (left.some((x) => x.turns.length)) remember<Conversation>(conversationKey(project), { refs: shownRefs.current, threads: left });
      else forget(conversationKey(project));
    },
  });
  const restoring = useRef(!!saved && !search.models);
  useEffect(() => {
    if (!restoring.current || !saved) return;
    restoring.current = false;
    setSearch({ models: saved.refs.join(",") });
  }, [saved, setSearch]);
  const [prompt, setPrompt] = useState(() => readDraft(project));
  const composer = useRef<HTMLTextAreaElement | null>(null);
  const changePrompt = (text: string) => {
    setPrompt(text);
    writeDraft(project, text);
  };

  const full = refs.length >= MAX_COLUMNS;
  const addColumn = () => {
    if (full || !first) return;
    add();
    updateRefs((current) => [...current, modelRef(callable.find((x) => !current.includes(modelRef(x))) ?? first)]);
  };
  const removeColumn = (i: number) => {
    remove(i);
    updateRefs((current) => current.filter((_, j) => j !== i));
  };
  const submit = (text: string) => {
    const sends = threads.flatMap((thread, i) => {
      const m = models[i];
      return m ? [{ column: i, request: requestFor(m, thread.turns, text, parameters) }] : [];
    });
    send(text, sends);
    changePrompt("");
  };
  const retryColumn = (i: number, m: PlaygroundModel, earlier: Turn[], last: Turn) => retry(i, requestFor(m, earlier, last.prompt, parameters));

  const noModels = catalogue.isSuccess && chat.length === 0;
  usePageCreateAction(noModels ? null : { label: t("playground.addColumn", "Add column"), run: addColumn, disabledReason: full ? fullReason() : undefined });

  const empty = threads.every((x) => x.turns.length === 0);
  useEffect(() => {
    if (empty) return;
    window.addEventListener("beforeunload", warnOnLeave);
    return () => window.removeEventListener("beforeunload", warnOnLeave);
  }, [empty]);
  const clearAll = () => {
    const before = clear();
    toast(t("playground.cleared", "Conversation cleared"), {
      id: CLEARED,
      action: {
        label: t("playground.undoClear", "Undo"),
        onClick: () => {
          restore(before);
          toast.dismiss(CLEARED);
        },
      },
    });
  };
  const options = useMemo(
    () =>
      chat.map((m) => playgroundModelOption(m, modelRef(m), t("playground.noProviderKeyShort", "No {provider} key", { provider: providers.name(m.provider) }))),
    [chat, providers],
  );
  const providerIds = useMemo(() => [...new Set(chat.map((m) => m.provider))], [chat]);
  const pickerProviders = useModelPickerProviders(org, providerIds);
  const chosen = models.filter((m): m is PlaygroundModel => !!m);
  const ready = !catalogue.isPending && chosen.length > 0;
  useEffect(() => {
    if (ready && window.matchMedia("(pointer: fine)").matches) composer.current?.focus();
  }, [ready]);
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
            {!empty && (
              <Button size="sm" variant="ghost" onClick={clearAll}>
                {common.clear()}
              </Button>
            )}
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
                        <ModelPicker
                          aria-label={common.model()}
                          appearance="ghost"
                          size="sm"
                          placeholder={catalogue.isPending ? t("playground.loadingModels", "Loading models") : t("playground.chooseModel", "Choose a model")}
                          value={m ? ref : null}
                          onValueChange={(v) => {
                            rememberModel(project, v);
                            updateRefs((current) => current.map((r, j) => (j === i ? v : r)));
                          }}
                          options={options}
                          providers={pickerProviders}
                          recent={recentModels(project)}
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
                      ) : empty && i === 0 ? (
                        <Empty
                          size="md"
                          className="my-auto"
                          title={t("playground.emptyThread", "Nothing sent yet")}
                          description={
                            refs.length > 1
                              ? t("playground.emptyThreadCompare", "Send a prompt below and every column answers it side by side.")
                              : t("playground.emptyThreadHint", "Send a prompt below, or start from one of the examples.")
                          }
                        />
                      ) : null}
                    </CompareGrid.Column>
                  );
                })}
              </CompareGrid>
            </div>
            <div className="mx-24 mt-12 mb-16 flex flex-col gap-8">
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
