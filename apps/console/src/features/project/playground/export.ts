import type { DecisionAnswer, DecisionQuestion } from "@logicplanes/kaeso-ui/playground";
import { formatDuration, formatNumber } from "@logicplanes/kaeso-ui/format";
import type { PlaygroundModel } from "~/api/queries/playground";
import { requestUSD } from "~/format/money";
import { t } from "~/i18n";
import { metricsOf } from "./metrics";
import type { ResponseState, Turn } from "./responses";

/** One column of the comparison, as the export writes it. */
export interface ExportColumn {
  /** The model as the page's address names it, such as `openai/gpt-5`. */
  ref: string;
  label: string;
  model: PlaygroundModel | null;
  turns: readonly Turn[];
}

export interface Conversation {
  /** The project's name, once the console has it. */
  project: string | undefined;
  /** The parameters from the page's address, such as `temperature`. */
  parameters: Readonly<Record<string, unknown>>;
  columns: readonly ExportColumn[];
  at: Date;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** `playground-<project>-2026-11-15-1430`, in the reader's own time. */
export function exportName(c: Conversation, extension: "md" | "json"): string {
  const d = c.at;
  const project =
    (c.project ?? "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "project";
  return `playground-${project}-${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}.${extension}`;
}

/** The page's columns that have turns: a column's model is gone from the catalogue when null. */
export function columnsOf(
  refs: readonly string[],
  models: readonly (PlaygroundModel | null)[],
  threads: readonly ({ turns: readonly Turn[] } | undefined)[],
): ExportColumn[] {
  return refs.flatMap((ref, i) => {
    const turns = threads[i]?.turns ?? [];
    const model = models[i] ?? null;
    return turns.length ? [{ ref, label: model?.display_name ?? ref, model, turns }] : [];
  });
}

function figures(r: ResponseState, model: PlaygroundModel | null, turn: Turn) {
  const m = metricsOf(r, model, turn.mode ?? "text");
  return {
    timeToFirstToken: m.timeToFirstToken,
    duration: m.duration,
    inputTokens: m.inputTokens,
    outputTokens: m.outputTokens,
    cost: m.cost,
  };
}

function answerText(q: DecisionQuestion, a: DecisionAnswer | null): string {
  if (!a) return t("playground.export.noAnswer", "no answer");
  switch (a.kind) {
    case "predicate":
      return t("playground.export.predicate", "yes, probability {p}", { p: formatNumber(a.probability, { maximumFractionDigits: 3 }) });
    case "choice":
      return q.options?.[a.option]?.value ?? String(a.option);
    case "score":
      return formatNumber(a.score, { maximumFractionDigits: 3 });
    case "box":
      return `top ${a.top}, bottom ${a.bottom}, left ${a.left}, right ${a.right}`;
    case "refusal":
      return a.reason ? t("playground.export.refusedBecause", "refused: {reason}", { reason: a.reason }) : t("playground.export.refused", "refused");
  }
}

function replyLines(turn: Turn): string[] {
  const r = turn.response;
  if (r.status === "failed") return [t("playground.export.failed", "Failed: {reason}", { reason: r.error ?? "" })];
  const lines: string[] = [];
  if (turn.mode === "decision")
    (turn.questions ?? []).forEach((q, i) => lines.push(`- ${q.name?.trim() || q.instructions || String(i + 1)}: ${answerText(q, r.answers?.[i] ?? null)}`));
  else if (r.media?.length) for (const m of r.media) lines.push(`- ${m.label ?? m.name} (${m.name})`);
  else if (r.text) lines.push(r.text);
  if (r.status === "stopped") lines.push(t("playground.export.stopped", "Stopped before the model finished."));
  return lines;
}

function figureLine(turn: Turn, model: PlaygroundModel | null): string {
  const f = figures(turn.response, model, turn);
  const parts = [
    ...(f.timeToFirstToken != null ? [t("playground.export.firstToken", "first token {time}", { time: formatDuration(f.timeToFirstToken) })] : []),
    ...(f.duration != null ? [t("playground.export.took", "took {time}", { time: formatDuration(f.duration) })] : []),
    ...(f.inputTokens != null && f.outputTokens != null
      ? [t("playground.export.tokens", "{input} in · {output} out", { input: formatNumber(f.inputTokens), output: formatNumber(f.outputTokens) })]
      : []),
    ...(f.cost != null ? [requestUSD(f.cost)] : []),
    ...(turn.response.requestId ? [t("playground.export.request", "request {id}", { id: turn.response.requestId })] : []),
  ];
  return parts.length ? `_${parts.join(" · ")}_` : "";
}

/** The comparison as Markdown: a section per column, then each turn's prompt, reply and figures. */
export function conversationMarkdown(c: Conversation): string {
  const out = [`# ${t("playground.export.title", "Playground conversation")}`, ""];
  const params = Object.entries(c.parameters).filter(([, v]) => v !== undefined && v !== null && v !== "");
  if (params.length) out.push(`${t("playground.export.parameters", "Parameters")}: ${params.map(([k, v]) => `\`${k}=${String(v)}\``).join(", ")}`, "");
  for (const column of c.columns) {
    out.push(`## ${column.label} (\`${column.ref}\`)`, "");
    column.turns.forEach((turn, i) => {
      out.push(`### ${t("playground.export.turn", "Turn {n}", { n: i + 1 })}`, "", `**${t("playground.export.prompt", "Prompt")}**`, "", turn.prompt, "");
      const files = turn.files ?? [];
      if (files.length) out.push(`${t("playground.export.attachments", "Attachments")}: ${files.map((f) => `${f.name} (${f.type})`).join(", ")}`, "");
      const lines = replyLines(turn);
      out.push(`**${t("playground.export.reply", "Reply")}**`, "", ...(lines.length ? lines : [t("playground.export.noReply", "No reply.")]), "");
      const line = figureLine(turn, column.model);
      if (line) out.push(line, "");
    });
  }
  return `${out.join("\n").trimEnd()}\n`;
}

/** The same as data, with each turn's request id, so the request log finds it later. */
export function conversationJson(c: Conversation): string {
  const doc = {
    exported_at: c.at.toISOString(),
    project: c.project ?? null,
    parameters: c.parameters,
    columns: c.columns.map((column) => ({
      model: column.ref,
      label: column.label,
      turns: column.turns.map((turn) => {
        const r = turn.response;
        const f = figures(r, column.model, turn);
        return {
          mode: turn.mode ?? "text",
          prompt: turn.prompt,
          attachments: (turn.files ?? []).map((file) => ({ name: file.name, type: file.type })),
          ...(turn.questions ? { questions: turn.questions } : {}),
          reply: {
            status: r.status,
            request_id: r.requestId,
            text: r.text,
            ...(r.reasoning ? { reasoning: r.reasoning } : {}),
            ...(r.error ? { error: r.error } : {}),
            ...(r.media?.length ? { media: r.media.map((m) => ({ name: m.name, label: m.label ?? null })) } : {}),
            ...(r.answers ? { answers: r.answers } : {}),
            time_to_first_token_ms: f.timeToFirstToken,
            duration_ms: f.duration,
            input_tokens: f.inputTokens,
            output_tokens: f.outputTokens,
            cost: f.cost,
          },
        };
      }),
    })),
  };
  return `${JSON.stringify(doc, null, 2)}\n`;
}

/** Saves text as a file through the browser; nothing is sent anywhere. */
export function saveText(name: string, text: string, type: string): void {
  const href = URL.createObjectURL(new Blob([text], { type }));
  const a = document.createElement("a");
  a.href = href;
  a.download = name;
  a.click();
  URL.revokeObjectURL(href);
}
