import type { DecisionAnswer, DecisionQuestion } from "@logicplanes/kaeso-ui/playground";
import type { PlaygroundModel } from "~/api/queries/playground";
import { postMedia } from "~/api/playground-media";
import { t } from "~/i18n";
import { DECISION_IMAGES, type TurnFile } from "./attachments";
import type { MediaContext, MediaRun } from "./media";
import { usageOf } from "./responses";

type Json = Record<string, unknown>;
const obj = (v: unknown): Json | null => (v && typeof v === "object" && !Array.isArray(v) ? (v as Json) : null);
const num = (v: unknown): number | null => (typeof v === "number" && Number.isFinite(v) ? v : null);

/** Whether a column asks its model the questions: a decision model always, a model that also chats while there are questions. */
export const asks = (m: PlaygroundModel, questions: readonly DecisionQuestion[]) => m.mode === "decision" || (!!m.decisions && questions.length > 0);

export interface EditorLimits {
  allowBox: boolean;
  maxQuestions?: number | undefined;
  maxOptions?: number | undefined;
  maxLevels?: number | undefined;
}

const least = (values: (number | null | undefined)[]) => {
  const known = values.filter((v): v is number => typeof v === "number");
  return known.length ? Math.min(...known) : undefined;
};

/** What the question editor offers: the strictest limits of the models that answer, and box only when every one draws boxes. */
export function editorLimits(models: readonly PlaygroundModel[]): EditorLimits {
  const answering = models.flatMap((m) => (m.decisions ? [m.decisions] : []));
  return {
    allowBox: answering.length > 0 && answering.every((d) => d.question_types.includes("box")),
    maxQuestions: least(answering.map((d) => d.max_questions)),
    maxOptions: least(answering.map((d) => d.max_options)),
    maxLevels: least(answering.map((d) => d.max_levels)),
  };
}

/** Each question's key in the request: its name, or `q1`… for one without, never the same twice. */
export function questionKeys(questions: readonly DecisionQuestion[]): string[] {
  const named = new Set(questions.map((q) => q.name?.trim()).filter(Boolean));
  let n = 0;
  return questions.map((q) => {
    const name = q.name?.trim();
    if (name) return name;
    let key: string;
    do key = `q${++n}`;
    while (named.has(key));
    return key;
  });
}

const optionValues = (q: DecisionQuestion) => (q.options ?? []).map((o) => o.value.trim());
const levelsOf = (q: DecisionQuestion) => q.levels ?? [];

const label = (q: DecisionQuestion, i: number) => q.name?.trim() || t("playground.decisions.question", "Question {index}", { index: i + 1 });

/** What keeps the questions from being sent, as one sentence, or `null` when they can go. */
export function problemOf(questions: readonly DecisionQuestion[]): string | null {
  const seen = new Set<string>();
  for (const [i, q] of questions.entries()) {
    const name = q.name?.trim();
    if (name) {
      if (seen.has(name)) return t("playground.decisions.sameName", "Two questions are named {name}; rename one.", { name });
      seen.add(name);
    }
    if (q.kind === "choice") {
      const values = optionValues(q);
      if (values.some((v) => !v)) return t("playground.decisions.emptyOption", "Give every option of {question} a value.", { question: label(q, i) });
      if (new Set(values).size !== values.length)
        return t("playground.decisions.sameOption", "Two options of {question} have the same value.", { question: label(q, i) });
    }
    if (q.kind === "score" && levelsOf(q).some((l) => !l.label?.trim() && !l.description?.trim()))
      return t("playground.decisions.emptyLevel", "Describe every level of {question}.", { question: label(q, i) });
  }
  return null;
}

const text = (v: string | undefined) => (v?.trim() ? v : undefined);

/** A question as System One writes it. */
export function systemOneQuestion(q: DecisionQuestion): Json {
  const instructions = text(q.instructions);
  const base = instructions ? { instructions } : {};
  switch (q.kind) {
    case "predicate": {
      const yes = text(q.whenTrue);
      const no = text(q.whenFalse);
      return { type: "noul", ...base, ...(yes || no ? { criteria: { ...(yes ? { true: yes } : {}), ...(no ? { false: no } : {}) } } : {}) };
    }
    case "choice":
      return { type: "choice", ...base, criteria: Object.fromEntries((q.options ?? []).map((o) => [o.value.trim(), text(o.description) ?? null])) };
    case "score":
      return {
        type: "score",
        ...base,
        criteria: levelsOf(q).map((l) => [text(l.label), text(l.description)].filter(Boolean).join(": ")),
      };
    case "box":
      return { type: "box", ...base };
  }
}

const reference = (i: number) => `<image-${i + 1}>`;

/** The state: JSON when the prompt is a JSON object or array, else its text; with images, a list of the text and each image's reference. */
export function stateOf(prompt: string, images: readonly TurnFile[]): unknown {
  if (images.length) return [...(prompt.trim() ? [prompt] : []), ...images.map((_, i) => reference(i))];
  const trimmed = prompt.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    try {
      return JSON.parse(trimmed) as unknown;
    } catch {
      return prompt;
    }
  }
  return prompt;
}

/** The System One request a column sends. */
export function decisionRequest(m: PlaygroundModel, questions: readonly DecisionQuestion[], prompt: string, images: readonly TurnFile[]): Json {
  const keys = questionKeys(questions);
  const assets = images.map((f, i) => {
    const [head = "", data = ""] = f.data.split(",", 2);
    const format = /^data:image\/jpe?g/.test(head) ? "jpeg" : "png";
    return [reference(i), { type: "image", format, data_base64: data }] as const;
  });
  return {
    model: m.model,
    state: stateOf(prompt, images),
    questions: Object.fromEntries(questions.map((q, i) => [keys[i], systemOneQuestion(q)])),
    ...(assets.length ? { assets: Object.fromEntries(assets) } : {}),
  };
}

function probabilitiesOf(given: Json | null, keys: readonly string[]): (number | null)[] {
  return keys.map((k) => num(given?.[k]));
}

/** One System One answer as the answer of the question it was asked for, `null` when it's missing or of another type. */
export function answerOf(q: DecisionQuestion, raw: unknown): DecisionAnswer | null {
  const a = obj(raw);
  if (!a) return null;
  const confidence = num(a.confidence);
  const sure = confidence === null ? {} : { confidence };
  if (a.type === "refusal") return { kind: "refusal", ...(typeof a.reason === "string" ? { reason: a.reason } : {}) };
  switch (q.kind) {
    case "predicate": {
      const p = num(a.noul);
      return a.type === "noul" && p !== null ? { kind: "predicate", probability: p, ...sure } : null;
    }
    case "choice": {
      const values = optionValues(q);
      const option = values.indexOf(String(a.choice));
      return a.type === "choice" && option >= 0 ? { kind: "choice", option, probabilities: probabilitiesOf(obj(a.probabilities), values), ...sure } : null;
    }
    case "score": {
      const score = num(a.score);
      const levels = levelsOf(q).map((_, i) => String(i));
      return a.type === "score" && score !== null ? { kind: "score", score, probabilities: probabilitiesOf(obj(a.probabilities), levels), ...sure } : null;
    }
    case "box": {
      const b = obj(a.box);
      const [top, bottom, left, right] = [num(b?.top), num(b?.bottom), num(b?.left), num(b?.right)];
      return a.type === "box" && top !== null && bottom !== null && left !== null && right !== null ? { kind: "box", top, bottom, left, right, ...sure } : null;
    }
  }
}

/** The answers of a System One reply, under each question's key, in the order of the questions sent. */
export function answersOf(answers: Json, questions: readonly DecisionQuestion[]): (DecisionAnswer | null)[] {
  const keys = questionKeys(questions);
  return questions.map((q, i) => answerOf(q, answers[String(keys[i])]));
}

async function json(blob: Blob): Promise<Json> {
  try {
    return obj(JSON.parse(await blob.text())) ?? {};
  } catch {
    return {};
  }
}

/** Asks one model the questions about the prompt and its images. */
export function decisionRun(
  ctx: MediaContext,
  m: PlaygroundModel,
  questions: readonly DecisionQuestion[],
  prompt: string,
  files: readonly TurnFile[],
): MediaRun {
  return async (signal, _report, start) => {
    const images = files.filter((f) => DECISION_IMAGES.includes(f.type));
    const answer = await postMedia(ctx.org, ctx.project, "decisions", decisionRequest(m, questions, prompt, images), signal);
    const reply = await json(answer.blob);
    const answers = obj(reply.answers);
    if (!answers)
      return {
        ...start,
        status: "failed",
        endedAt: ctx.now(),
        requestId: answer.requestId,
        error: t("playground.decisions.unreadable", "The model's answers couldn't be read. Try again."),
      };
    return {
      ...start,
      status: "completed",
      endedAt: ctx.now(),
      cost: answer.cost,
      costKnown: answer.costKnown,
      requestId: answer.requestId,
      usage: usageOf(reply),
      answers: answersOf(answers, questions),
    };
  };
}
