import { forwardRef, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText, type UiText } from "../../i18n/context.js";
import { Button } from "../button/button.js";
import { Input } from "../input/input.js";
import { Textarea } from "../input/textarea.js";
import { Select } from "../select/select.js";
import { Separator } from "../separator/separator.js";
import { kindLabel, levelLabel, questionName, type DecisionKind, type DecisionLevel, type DecisionOption, type DecisionQuestion } from "./decision-types.js";

export const KAESO_QUESTION_EDITOR_VARIANTS = {
  layout: {
    panel: { classes: "gap-24", description: "Questions 24px apart, in a side panel or a sheet (default)" },
    compact: { classes: "gap-16", description: "Questions 16px apart, inside a popover or a narrow column" },
  },
} as const;
export const KAESO_QUESTION_EDITOR_DEFAULT_VARIANTS = { layout: "panel" } as const;

export interface QuestionEditorProps {
  value: readonly DecisionQuestion[];
  onValueChange: (next: DecisionQuestion[]) => void;
  allowBox?: boolean | undefined;
  maxQuestions?: number | undefined;
  maxOptions?: number | undefined;
  maxLevels?: number | undefined;
  disabled?: boolean | undefined;
  layout?: keyof typeof KAESO_QUESTION_EDITOR_VARIANTS.layout | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

const MIN_ROWS = 2;
const NEW_LEVELS = 3;

function text(v: string): string | undefined {
  return v === "" ? undefined : v;
}

function withKind(q: DecisionQuestion, kind: DecisionKind): DecisionQuestion {
  const next: DecisionQuestion = { ...q, kind };
  if (kind === "choice" && !q.options?.length) next.options = Array.from({ length: MIN_ROWS }, () => ({ value: "" }));
  if (kind === "score" && !q.levels?.length) next.levels = Array.from({ length: NEW_LEVELS }, () => ({}));
  return next;
}

function kinds(t: UiText, allowBox: boolean, current: DecisionKind) {
  const all: DecisionKind[] = ["predicate", "choice", "score", "box"];
  return all.filter((k) => k !== "box" || allowBox || current === "box").map((k) => ({ value: k, label: kindLabel(t, k) }));
}

interface RowsProps<T> {
  rows: readonly T[];
  idOf: (i: number) => string;
  min: number;
  max: number;
  disabled: boolean;
  first: (row: T, i: number) => { value: string; label: string; placeholder: string };
  second: (row: T, i: number) => { value: string; label: string; placeholder: string };
  removeLabel: (i: number) => string;
  addLabel: string;
  limitText: string;
  onChange: (rows: T[]) => void;
  onFocus: (id: string) => void;
  make: () => T;
  setFirst: (row: T, v: string) => T;
  setSecond: (row: T, v: string) => T;
}

function Rows<T>({
  rows,
  idOf,
  min,
  max,
  disabled,
  first,
  second,
  removeLabel,
  addLabel,
  limitText,
  onChange,
  onFocus,
  make,
  setFirst,
  setSecond,
}: RowsProps<T>) {
  const full = rows.length >= max;
  const insert = (at: number) => {
    if (full) return;
    onChange([...rows.slice(0, at), make(), ...rows.slice(at)]);
    onFocus(idOf(at));
  };
  const remove = (i: number) => {
    onChange(rows.filter((_, j) => j !== i));
    onFocus(idOf(Math.max(0, i - 1)));
  };
  const enter = (i: number) => (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key !== "Enter" || e.nativeEvent.isComposing) return;
    e.preventDefault();
    insert(i + 1);
  };
  return (
    <div className="flex flex-col gap-8">
      <ul className="flex flex-col gap-16 sm:gap-8">
        {rows.map((row, i) => {
          const a = first(row, i);
          const b = second(row, i);
          return (
            <li key={i} className="flex min-w-0 items-start gap-8">
              <div className="grid min-w-0 flex-1 grid-cols-1 gap-8 sm:grid-cols-2">
                <Input
                  id={idOf(i)}
                  size="sm"
                  aria-label={a.label}
                  placeholder={a.placeholder}
                  value={a.value}
                  disabled={disabled}
                  onChange={(e) => onChange(rows.map((r, j) => (j === i ? setFirst(r, e.target.value) : r)))}
                  onKeyDown={enter(i)}
                />
                <Input
                  size="sm"
                  aria-label={b.label}
                  placeholder={b.placeholder}
                  value={b.value}
                  disabled={disabled}
                  onChange={(e) => onChange(rows.map((r, j) => (j === i ? setSecond(r, e.target.value) : r)))}
                />
              </div>
              <Button
                size="sm"
                variant="ghost"
                icon={icons.delete}
                aria-label={removeLabel(i)}
                disabled={disabled || rows.length <= min}
                onClick={() => remove(i)}
              />
            </li>
          );
        })}
      </ul>
      <div className="flex min-h-32 flex-wrap items-center gap-x-12 gap-y-4">
        <Button size="sm" variant="secondary" leadingIcon={icons.add} disabled={disabled || full} onClick={() => insert(rows.length)}>
          {addLabel}
        </Button>
        {full && <span className="text-small text-fg-tertiary">{limitText}</span>}
      </div>
    </div>
  );
}

export const QuestionEditor = forwardRef<HTMLDivElement, QuestionEditorProps>(function QuestionEditor(
  {
    value,
    onValueChange,
    allowBox = false,
    maxQuestions = Infinity,
    maxOptions = 255,
    maxLevels = 10,
    disabled = false,
    layout = "panel",
    "aria-label": ariaLabel,
    className,
  },
  ref,
) {
  const t = useUiText();
  const base = useId();
  const counter = useRef(0);
  const [focus, setFocus] = useState<string | null>(null);
  useEffect(() => {
    if (!focus) return;
    document.getElementById(focus)?.focus();
    setFocus(null);
  }, [focus, value]);

  const keyOf = (q: DecisionQuestion, i: number) => q.id ?? `${base}-i${i}`;
  const update = (i: number, q: DecisionQuestion) => onValueChange(value.map((old, j) => (j === i ? q : old)));
  const add = () => {
    if (value.length >= maxQuestions) return;
    counter.current += 1;
    const id = `${base}-q${counter.current}`;
    onValueChange([...value, { id, kind: "predicate" }]);
    setFocus(`${id}-name`);
  };
  const remove = (i: number) => {
    const next = value.filter((_, j) => j !== i);
    onValueChange(next);
    const neighbour = next[Math.min(i, next.length - 1)];
    setFocus(neighbour ? `${keyOf(neighbour, Math.min(i, next.length - 1))}-remove` : `${base}-add`);
  };

  return (
    <div
      ref={ref}
      role="group"
      aria-label={ariaLabel ?? t("questionEditor.label", "Questions")}
      className={cn("flex min-w-0 flex-col", KAESO_QUESTION_EDITOR_VARIANTS.layout[layout].classes, className)}
    >
      {value.length === 0 && (
        <p className="text-body text-fg-secondary">
          {t("questionEditor.empty", "No questions yet. Add one to ask the model something it answers with probabilities.")}
        </p>
      )}
      {value.length > 0 && (
        <ol className={cn("flex min-w-0 flex-col", KAESO_QUESTION_EDITOR_VARIANTS.layout[layout].classes)}>
          {value.map((q, i) => {
            const key = keyOf(q, i);
            const title = questionName(t, q, i);
            const headId = `${key}-head`;
            return (
              <li key={key} className={cn("flex min-w-0 flex-col", KAESO_QUESTION_EDITOR_VARIANTS.layout[layout].classes)}>
                {i > 0 && <Separator aria-hidden />}
                <fieldset aria-labelledby={headId} className="flex min-w-0 flex-col gap-12" disabled={disabled}>
                  <div className="flex min-h-32 min-w-0 items-center justify-between gap-12">
                    <span id={headId} className="min-w-0 text-body font-medium text-fg wrap-anywhere">
                      {title}
                    </span>
                    <Button
                      id={`${key}-remove`}
                      size="sm"
                      variant="ghost"
                      icon={icons.delete}
                      aria-label={t("questionEditor.removeQuestion", "Remove {name}", { name: title })}
                      disabled={disabled}
                      onClick={() => remove(i)}
                    />
                  </div>
                  <div className="grid min-w-0 grid-cols-1 gap-12 sm:grid-cols-2">
                    <Select
                      label={t("questionEditor.kind", "Type")}
                      value={q.kind}
                      options={kinds(t, allowBox, q.kind)}
                      disabled={disabled}
                      fullWidth
                      onValueChange={(k) => update(i, withKind(q, k as DecisionKind))}
                    />
                    <Input
                      id={`${key}-name`}
                      label={t("questionEditor.name", "Name")}
                      placeholder={t("questionEditor.namePlaceholder", "is_invoice")}
                      value={q.name ?? ""}
                      disabled={disabled}
                      onChange={(e) => update(i, { ...q, name: text(e.target.value) })}
                    />
                  </div>
                  <Textarea
                    label={t("questionEditor.instructions", "Instructions")}
                    placeholder={t("questionEditor.instructionsPlaceholder", "What the model should decide, in a sentence")}
                    rows={2}
                    value={q.instructions ?? ""}
                    disabled={disabled}
                    onChange={(e) => update(i, { ...q, instructions: text(e.target.value) })}
                  />
                  {q.kind === "predicate" && (
                    <div className="grid min-w-0 grid-cols-1 gap-12 sm:grid-cols-2">
                      <Input
                        label={t("questionEditor.whenTrue", "What yes means")}
                        value={q.whenTrue ?? ""}
                        disabled={disabled}
                        onChange={(e) => update(i, { ...q, whenTrue: text(e.target.value) })}
                      />
                      <Input
                        label={t("questionEditor.whenFalse", "What no means")}
                        value={q.whenFalse ?? ""}
                        disabled={disabled}
                        onChange={(e) => update(i, { ...q, whenFalse: text(e.target.value) })}
                      />
                    </div>
                  )}
                  {q.kind === "choice" && (
                    <div role="group" aria-labelledby={`${key}-options`} className="flex min-w-0 flex-col gap-8">
                      <span id={`${key}-options`} className="text-body font-medium text-fg">
                        {t("questionEditor.options", "Options")}
                      </span>
                      <Rows<DecisionOption>
                        rows={q.options ?? []}
                        idOf={(n) => `${key}-option-${n}`}
                        min={MIN_ROWS}
                        max={maxOptions}
                        disabled={disabled}
                        first={(o, n) => ({
                          value: o.value,
                          label: t("questionEditor.optionValue", "Option {n}", { n: n + 1 }),
                          placeholder: t("questionEditor.optionPlaceholder", "Value"),
                        })}
                        second={(o, n) => ({
                          value: o.description ?? "",
                          label: t("questionEditor.optionDescription", "What option {n} means", { n: n + 1 }),
                          placeholder: t("questionEditor.descriptionPlaceholder", "What it means"),
                        })}
                        removeLabel={(n) => t("questionEditor.removeOption", "Remove option {n}", { n: n + 1 })}
                        addLabel={t("questionEditor.addOption", "Add option")}
                        limitText={t("questionEditor.optionLimit", "{max, plural, one {Up to # option} other {Up to # options}}", { max: maxOptions })}
                        onChange={(options) => update(i, { ...q, options })}
                        onFocus={setFocus}
                        make={() => ({ value: "" })}
                        setFirst={(o, v) => ({ ...o, value: v })}
                        setSecond={(o, v) => ({ ...o, description: text(v) })}
                      />
                    </div>
                  )}
                  {q.kind === "score" && (
                    <div role="group" aria-labelledby={`${key}-levels`} className="flex min-w-0 flex-col gap-8">
                      <span id={`${key}-levels`} className="text-body font-medium text-fg">
                        {t("questionEditor.levels", "Levels, from 0 up")}
                      </span>
                      <Rows<DecisionLevel>
                        rows={q.levels ?? []}
                        idOf={(n) => `${key}-level-${n}`}
                        min={MIN_ROWS}
                        max={maxLevels}
                        disabled={disabled}
                        first={(l, n) => ({
                          value: l.label ?? "",
                          label: t("questionEditor.levelLabel", "Label of level {n}", { n }),
                          placeholder: levelLabel(t, undefined, n),
                        })}
                        second={(l, n) => ({
                          value: l.description ?? "",
                          label: t("questionEditor.levelDescription", "What level {n} means", { n }),
                          placeholder: t("questionEditor.descriptionPlaceholder", "What it means"),
                        })}
                        removeLabel={(n) => t("questionEditor.removeLevel", "Remove level {n}", { n })}
                        addLabel={t("questionEditor.addLevel", "Add level")}
                        limitText={t("questionEditor.levelLimit", "{max, plural, one {Up to # level} other {Up to # levels}}", { max: maxLevels })}
                        onChange={(levels) => update(i, { ...q, levels })}
                        onFocus={setFocus}
                        make={() => ({})}
                        setFirst={(l, v) => ({ ...l, label: text(v) })}
                        setSecond={(l, v) => ({ ...l, description: text(v) })}
                      />
                    </div>
                  )}
                  {q.kind === "box" && (
                    <p className="text-small text-fg-secondary">
                      {t("questionEditor.boxHint", "The model answers with a box around what the instructions describe, on the image sent with the state.")}
                    </p>
                  )}
                </fieldset>
              </li>
            );
          })}
        </ol>
      )}
      <div className="flex min-h-36 flex-wrap items-center gap-x-12 gap-y-4">
        <Button id={`${base}-add`} variant="secondary" leadingIcon={icons.add} disabled={disabled || value.length >= maxQuestions} onClick={add}>
          {t("questionEditor.addQuestion", "Add question")}
        </Button>
        {value.length >= maxQuestions && (
          <span className="text-small text-fg-tertiary">
            {t("questionEditor.questionLimit", "{max, plural, one {Up to # question} other {Up to # questions}}", { max: maxQuestions })}
          </span>
        )}
      </div>
    </div>
  );
});
QuestionEditor.displayName = "QuestionEditor";
