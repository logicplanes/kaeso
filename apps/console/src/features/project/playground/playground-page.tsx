import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { AppFrame, Button, Empty, Sheet, Text, icons } from "@logicplanes/kaeso-ui";
import { CompareGrid, Composer, ModelPicker, ParameterPanel, QuestionEditor, type DecisionQuestion } from "@logicplanes/kaeso-ui/playground";
import { playgroundModelsQuery, type PlaygroundModel } from "~/api/queries/playground";
import { useProjectRights } from "~/auth/rights";
import { playgroundModelOption, recentModels, rememberModel } from "~/kaeso/model-options";
import { useModelPickerProviders, useProviders } from "~/kaeso/providers";
import { usePageCreateAction } from "~/shell/create-action";
import { ProjectHeader } from "~/shell/project-header";
import { modelRef } from "./request";
import { parametersOf, searchOf } from "./search";
import { MAX_COLUMNS, parseModels, validatePlaygroundSearch, type PlaygroundSearch } from "~/format/playground-search";
import Thread from "./thread";
import { isLive } from "./responses";
import { useThreads, type Thread as ThreadState } from "./use-threads";
import { locale, t } from "~/i18n";
import { common } from "~/i18n/common";
import { readStored, writeStored } from "~/storage";
import { PLAYGROUND_CONVERSATION, playgroundDraftKey } from "~/auth/user-data";
import { forget, recall, remember } from "~/shell/session-memory";
import { toast } from "~/shell/toast";
import { patchSearch, type SearchPatch } from "~/search";
import { acceptOf, fileId, fileRules, readFile, shown, sizeOf, type TurnFile } from "./attachments";
import { imagesExpected, modeLabel, panelOptions, sendFor } from "./modes";
import { canRecord, useRecorder } from "./use-recorder";
import { asks, editorLimits, problemOf } from "./decisions";

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
const questionsKey = (project: string) => [...PLAYGROUND_CONVERSATION, project, "questions"];
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

  // A new column starts with a chat model, so a project with media models still opens on a conversation.
  const callable = chat
    .filter((m) => m.provider_key && !m.needs_api_key && m.availability !== "unavailable")
    .sort((a, b) => Number(a.mode !== "text") - Number(b.mode !== "text"));
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
  const { threads, busy, send, retry, stop, cancel, add, remove, clear, restore } = useThreads(org, project, refs.length, {
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
  const [files, setFiles] = useState<TurnFile[]>([]);
  const waiting = useMemo(() => shown(files), [files]);
  const [questions, setQuestionsNow] = useState<DecisionQuestion[]>(() => recall<DecisionQuestion[]>(questionsKey(project)) ?? []);
  const setQuestions = (next: DecisionQuestion[]) => {
    setQuestionsNow(next);
    remember(questionsKey(project), next);
  };
  const [editing, setEditing] = useState(false);
  const media = { org, project, now: () => performance.now() };
  const submit = (text: string) => {
    const asked = models.find((m): m is PlaygroundModel => !!m && asks(m, questions));
    const problem = !asked
      ? null
      : questions.length
        ? problemOf(questions)
        : t("playground.decisions.none", "Add a question for {model} to answer.", { model: asked.display_name });
    if (problem) {
      toast.error(problem);
      setEditing(true);
      return;
    }
    const sends = threads.flatMap((thread, i) => {
      const m = models[i];
      return m ? [{ column: i, ...sendFor(media, m, thread.turns, text, files, parameters, questions) }] : [];
    });
    send(text, sends);
    changePrompt("");
    setFiles([]);
  };
  const latest = useRef({ threads, models, parameters, retry, media, questions });
  useEffect(() => {
    latest.current = { threads, models, parameters, retry, media, questions };
  });
  const retryThread = useCallback((key: string) => {
    const now = latest.current;
    const i = now.threads.findIndex((x) => x.key === key);
    const m = now.models[i];
    const thread = now.threads[i];
    const last = thread?.turns.at(-1);
    if (m && thread && last)
      now.retry(i, sendFor(now.media, m, thread.turns.slice(0, -1), last.prompt, last.files ?? [], now.parameters, last.questions ?? []));
  }, []);

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
      chat.map((m) => {
        const option = playgroundModelOption(m, modelRef(m), t("playground.noProviderKeyShort", "No {provider} key", { provider: providers.name(m.provider) }));
        return option.note || m.mode === "text" ? option : { ...option, note: modeLabel(m.mode) };
      }),
    [chat, providers],
  );
  const providerIds = useMemo(() => [...new Set(chat.map((m) => m.provider))], [chat]);
  const pickerProviders = useModelPickerProviders(org, providerIds);
  const chosen = models.filter((m): m is PlaygroundModel => !!m);
  const ready = !catalogue.isPending && chosen.length > 0;
  useEffect(() => {
    if (ready && window.matchMedia("(pointer: fine)").matches) composer.current?.focus();
  }, [ready]);
  const rules = fileRules(chosen);
  const attach = async (picked: File[], label?: string) => {
    const sent = Math.max(0, ...threads.map((x) => sizeOf(x.turns.flatMap((turn) => turn.files ?? []))));
    let used = sent + sizeOf(files);
    const added: TurnFile[] = [];
    const refused: string[] = [];
    for (const file of picked) {
      if (!rules.takes(file.type)) {
        refused.push(t("playground.fileType", "{name} isn't a file these models take.", { name: file.name }));
        continue;
      }
      let data: string;
      try {
        data = await readFile(file);
      } catch {
        refused.push(t("playground.fileUnreadable", "{name} couldn't be read; try it again.", { name: file.name }));
        continue;
      }
      if (used + data.length > rules.maxBytes(file.type)) {
        refused.push(t("playground.fileTooLarge", "{name} makes the conversation too large to send.", { name: file.name }));
        continue;
      }
      used += data.length;
      added.push({ id: fileId(), name: label ?? file.name, type: file.type, data, ...(label ? { filename: file.name } : {}) });
    }
    if (added.length) setFiles((now) => [...now, ...added]);
    if (refused.length === 1) toast.error(refused[0]);
    else if (refused.length > 1) toast.error(t("playground.filesRefused", "{count} files were too large or not taken.", { count: refused.length }));
  };
  const panel = panelOptions(chosen);
  const modes = new Set(chosen.map((m) => m.mode));
  const recorder = useRecorder(
    (file) =>
      void attach(
        [file],
        t("playground.recordingName", "Recording {time}", {
          time: new Date().toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit", second: "2-digit" }),
        }),
      ),
    () => toast.error(t("playground.microphoneRefused", "The microphone isn't available; allow it in the browser.")),
  );
  const recordable = modes.has("transcription") && canRecord();
  const attachLabel = rules.types.some((x) => x.startsWith("audio/"))
    ? rules.types.length > 1
      ? t("playground.attachFiles", "Attach files")
      : t("playground.attachAudio", "Attach audio")
    : rules.types.includes("application/pdf")
      ? rules.types.length > 1
        ? t("playground.attachImagesPdfs", "Attach images or PDFs")
        : t("playground.attachPdfs", "Attach PDFs")
      : t("playground.attachImages", "Attach images");
  const deciders = chosen.filter((m) => m.decisions);
  const asking = chosen.some((m) => asks(m, questions));
  const limits = editorLimits(deciders);
  const placeholder =
    modes.size === 1 && modes.has("transcription")
      ? t("playground.promptAudio", "Attach or record audio, and add a note if it helps")
      : asking
        ? refs.length > 1
          ? t("playground.promptStateEvery", "Send what the questions are about to every model")
          : t("playground.promptState", "Describe what the questions are about")
        : refs.length > 1
          ? t("playground.promptEvery", "Send a prompt to every model")
          : t("playground.prompt", "Send a prompt");

  return (
    <>
      <ProjectHeader
        org={org}
        project={project}
        page={t("playground.title", "Playground")}
        path="playground"
        actions={
          !empty ? (
            <Button size="sm" variant="ghost" onClick={clearAll}>
              {common.clear()}
            </Button>
          ) : undefined
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
              <CompareGrid aria-label={common.models()} onAdd={addColumn} addLabel={t("playground.addColumn", "Add column")} max={MAX_COLUMNS}>
                {refs.map((ref, i) => {
                  const m = models[i] ?? null;
                  const thread = threads[i];
                  const lastTurn = thread?.turns.at(-1);
                  const label = m?.display_name ?? common.model();
                  return (
                    <CompareGrid.Column
                      key={thread?.key ?? `col${i}`}
                      label={label}
                      followKey={lastTurn?.id}
                      header={
                        <ModelPicker
                          aria-label={common.model()}
                          appearance="ghost"
                          size="sm"
                          placeholder={
                            catalogue.isPending
                              ? t("playground.loadingModels", "Loading models")
                              : catalogue.isError
                                ? t("playground.modelsFailed", "The models couldn't load")
                                : t("playground.chooseModel", "Choose a model")
                          }
                          value={m ? ref : null}
                          onValueChange={(v) => {
                            rememberModel(project, v);
                            updateRefs((current) => current.map((r, j) => (j === i ? v : r)));
                          }}
                          options={options}
                          providers={pickerProviders}
                          recent={recentModels(project)}
                          disabled={catalogue.isPending || catalogue.isError}
                        />
                      }
                      onRemove={() => removeColumn(i)}
                      removeLabel={t("playground.removeColumn", "Remove {model}", { model: label })}
                    >
                      {thread && lastTurn ? (
                        <Thread
                          className="mx-auto w-full max-w-800"
                          turns={thread.turns}
                          model={m}
                          label={label}
                          threadKey={thread.key}
                          expected={imagesExpected(m, parameters)}
                          onRetry={m && !isLive(lastTurn.response) ? retryThread : null}
                          onCancel={cancel}
                        />
                      ) : empty ? (
                        i === 0 && (
                          <Text as="p" variant="body" tone="secondary" className="m-auto max-w-prose text-center text-pretty">
                            {t("playground.emptyThread", "Send a prompt below to start.")}
                          </Text>
                        )
                      ) : (
                        <Text as="p" variant="body" tone="secondary" className="m-auto max-w-prose text-center text-pretty">
                          {t("playground.emptyColumn", "Answers appear here from the next prompt you send.")}
                        </Text>
                      )}
                    </CompareGrid.Column>
                  );
                })}
              </CompareGrid>
            </div>
            <div className="mx-24 mt-4 mb-24 flex shrink-0 justify-center">
              <div className="flex w-full max-w-800 min-w-0 flex-col gap-12">
                <Composer
                  ref={composer}
                  size="sm"
                  value={prompt}
                  onValueChange={changePrompt}
                  onSubmit={submit}
                  onStop={stop}
                  streaming={busy}
                  disabled={catalogue.isPending || chosen.length === 0}
                  placeholder={placeholder}
                  attachments={waiting}
                  onAttach={rules.types.length ? (picked) => void attach(picked) : undefined}
                  onRemoveAttachment={(id) => setFiles((now) => now.filter((f) => f.id !== id))}
                  accept={acceptOf(rules)}
                  attachLabel={attachLabel}
                  actions={
                    <>
                      <ParameterPanel
                        value={parameters}
                        onValueChange={setParameters}
                        {...panel}
                        side="top"
                        align="start"
                        trigger={
                          <Button
                            size="sm"
                            variant="ghost"
                            icon={icons.displayOptions}
                            aria-label={t("playground.parameters", "Parameters")}
                            disabled={noModels}
                          />
                        }
                      />
                      {deciders.length > 0 && (
                        <Sheet
                          open={editing}
                          onOpenChange={setEditing}
                          modal={false}
                          size="md"
                          title={t("playground.decisions.title", "Questions")}
                          trigger={
                            <Button size="sm" variant="ghost" leadingIcon={icons.list} pressed={editing}>
                              {questions.length
                                ? t("playground.decisions.count", "Questions ({count})", { count: questions.length })
                                : t("playground.decisions.title", "Questions")}
                            </Button>
                          }
                        >
                          <Text as="p" variant="small" tone="secondary" className="mb-16">
                            {t("playground.decisions.subtitle", "Models that make decisions answer these about what you send.")}
                          </Text>
                          <QuestionEditor
                            value={questions}
                            onValueChange={setQuestions}
                            allowBox={limits.allowBox}
                            maxQuestions={limits.maxQuestions}
                            maxOptions={limits.maxOptions}
                            maxLevels={limits.maxLevels}
                          />
                        </Sheet>
                      )}
                      {recordable && (
                        <Button
                          size="sm"
                          variant="ghost"
                          icon={recorder.recording ? icons.stop : icons.record}
                          aria-label={recorder.recording ? t("playground.stopRecording", "Stop recording") : t("playground.record", "Record audio")}
                          pressed={recorder.recording}
                          onClick={() => (recorder.recording ? recorder.stop() : void recorder.start())}
                        />
                      )}
                    </>
                  }
                />
              </div>
            </div>
          </div>
        )}
      </AppFrame.Body>
    </>
  );
}
