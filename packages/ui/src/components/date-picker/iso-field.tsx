import { useState } from "react";
import { InputGroup } from "../input/input-group.js";
import { formatISODate, parseISODate } from "../../format/index.js";
import { useUiText, type UiText } from "../../i18n/context.js";

export type IsoProblem = "format" | "unavailable" | "missing" | null;

export function IsoField({
  label,
  value,
  onCommit,
  onProblem,
  errorId,
  invalid,
}: {
  label: string;
  value: Date | undefined;
  onCommit: (d: Date) => boolean | void;
  onProblem: (problem: IsoProblem) => void;
  errorId: string;
  invalid: boolean;
}) {
  const t = useUiText();
  const [text, setText] = useState(value ? formatISODate(value) : "");
  const [shown, setShown] = useState(value);
  if (value !== shown) {
    setShown(value);
    setText(value ? formatISODate(value) : "");
  }
  const commit = () => {
    const typed = text.trim();
    if (!typed) {
      setText(value ? formatISODate(value) : "");
      onProblem(null);
      return;
    }
    const d = parseISODate(typed);
    if (!d) return onProblem("format");
    if (onCommit(d) === false) return onProblem("unavailable");
    onProblem(null);
  };
  return (
    <InputGroup size="sm" className="flex-1" invalid={invalid}>
      <InputGroup.Input
        aria-label={label}
        placeholder={t("datePicker.isoPlaceholder", "YYYY-MM-DD")}
        value={text}
        inputMode="numeric"
        className="mono text-small"
        {...(invalid ? { "aria-invalid": true, "aria-describedby": errorId } : {})}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key !== "Enter") return;
          e.preventDefault();
          commit();
        }}
      />
    </InputGroup>
  );
}

export const ISO_PROBLEM = "mt-6 text-small text-pretty text-danger-text empty:mt-0";

export function isoProblemText(t: UiText, problem: IsoProblem): string {
  if (problem === "format") return t("datePicker.typeIso", "Enter the date as YYYY-MM-DD.");
  if (problem === "unavailable") return t("datePicker.unavailableDate", "This date can't be chosen. Choose another date.");
  if (problem === "missing") return t("datePicker.startFirst", "Choose a start date first.");
  return "";
}
