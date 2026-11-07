import { forwardRef, useRef } from "react";
import { cn } from "../../utils/cn.js";
import { useUiText } from "../../i18n/context.js";
import { useTruncated } from "../../hooks/use-truncated.js";
import { Tooltip } from "../tooltip/tooltip.js";

export const KAESO_PROMPT_SUGGESTIONS_VARIANTS = {
  align: {
    start: { classes: "justify-start", description: "Chips from the composer's edge, wrapping (default)" },
    center: { classes: "justify-center", description: "Centered under an empty thread's title" },
  },
} as const;
export const KAESO_PROMPT_SUGGESTIONS_DEFAULT_VARIANTS = { align: "start" } as const;

export interface PromptSuggestionsProps {
  suggestions: string[];
  onSelect: (prompt: string) => void;
  "aria-label"?: string | undefined;
  align?: keyof typeof KAESO_PROMPT_SUGGESTIONS_VARIANTS.align | undefined;
  className?: string | undefined;
  "data-force"?: string | undefined;
}

function Suggestion({ prompt, force, onSelect }: { prompt: string; force: string | undefined; onSelect: (prompt: string) => void }) {
  const text = useRef<HTMLSpanElement | null>(null);
  const truncated = useTruncated(text);
  return (
    <li className="flex max-w-full min-w-0">
      <Tooltip content={prompt} disabled={!truncated} shape="multiline">
        <button
          type="button"
          {...(force ? { "data-force": force } : {})}
          onClick={() => onSelect(prompt)}
          className="flex h-32 max-w-full min-w-0 cursor-pointer items-center rounded-pill px-12 text-body text-fg-secondary material-ghost shadow-ring transition-control hover:text-fg focus-visible:text-fg active:text-fg focus-ring coarse:h-44"
        >
          <span ref={text} className="min-w-0 truncate">
            {prompt}
          </span>
        </button>
      </Tooltip>
    </li>
  );
}

export const PromptSuggestions = forwardRef<HTMLUListElement, PromptSuggestionsProps>(function PromptSuggestions(
  { suggestions, onSelect, "aria-label": ariaLabelProp, align = "start", className, "data-force": force },
  ref,
) {
  const t = useUiText();
  const ariaLabel = ariaLabelProp ?? t("promptSuggestions.examplePrompts", "Example prompts");
  return (
    <ul ref={ref} aria-label={ariaLabel} className={cn("flex min-w-0 flex-wrap gap-8", KAESO_PROMPT_SUGGESTIONS_VARIANTS.align[align].classes, className)}>
      {suggestions.slice(0, 3).map((s, i) => (
        <Suggestion key={i} prompt={s} force={force} onSelect={onSelect} />
      ))}
    </ul>
  );
});
PromptSuggestions.displayName = "PromptSuggestions";
