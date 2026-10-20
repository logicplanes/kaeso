import { useState } from "react";
import { InputGroup } from "../input/input-group.js";
import { formatISODate, parseISODate } from "../../format/index.js";
import { useUiText } from "../../i18n/context.js";

export function IsoField({ label, value, onCommit }: { label: string; value: Date | undefined; onCommit: (d: Date) => boolean | void }) {
  const t = useUiText();
  const [text, setText] = useState(value ? formatISODate(value) : "");
  const [shown, setShown] = useState(value);
  if (value !== shown) {
    setShown(value);
    setText(value ? formatISODate(value) : "");
  }
  return (
    <InputGroup size="sm" className="flex-1">
      <InputGroup.Input
        aria-label={label}
        placeholder={t("datePicker.isoPlaceholder", "YYYY-MM-DD")}
        value={text}
        inputMode="numeric"
        className="mono text-small"
        onChange={(e) => setText(e.target.value)}
        onBlur={() => {
          const d = parseISODate(text);
          if (!d || onCommit(d) === false) setText(value ? formatISODate(value) : "");
        }}
        onKeyDown={(e) => {
          if (e.key !== "Enter") return;
          e.preventDefault();
          (e.target as HTMLInputElement).blur();
        }}
      />
    </InputGroup>
  );
}
