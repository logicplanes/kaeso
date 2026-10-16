import { forwardRef } from "react";
import { cn } from "../../utils/cn";
import { useUiText } from "../../i18n/context";

export const KAESO_PROMPT_SUGGESTIONS_VARIANTS = {
  align: {
    start: { classes: "items-stretch", description: "Rows the composer's width, their text on its text (default)" },
    center: { classes: "items-center text-center", description: "Centered under an empty thread's title" },
  },
} as const;
export const KAESO_PROMPT_SUGGESTIONS_DEFAULT_VARIANTS = { align: "start" } as const;

export interface PromptSuggestionsProps {
  suggestions: string[];
  onSelect: (prompt: string) => void;
  "aria-label"?: string | undefined;
  align?: keyof typeof KAESO_PROMPT_SUGGESTIONS_VARIANTS.align | undefined;
  className?: string | undefined;
}

export const PromptSuggestions = forwardRef<HTMLUListElement, PromptSuggestionsProps>(function PromptSuggestions(
  { suggestions, onSelect, "aria-label": ariaLabelProp, align = "start", className },
  ref,
) {
  const t = useUiText();
  const ariaLabel = ariaLabelProp ?? t("promptSuggestions.examplePrompts", "Example prompts");
  return (
    <ul ref={ref} aria-label={ariaLabel} className={cn("flex min-w-0 flex-col gap-2", KAESO_PROMPT_SUGGESTIONS_VARIANTS.align[align].classes, className)}>
      {suggestions.slice(0, 3).map((s) => (
        <li key={s} className={cn("flex min-w-0", align === "center" && "justify-center")}>
          <button
            type="button"
            onClick={() => onSelect(s)}
            className={cn(
              "flex h-36 max-w-full min-w-0 cursor-pointer items-center rounded-item px-16 text-body text-fg-secondary transition-control hover:bg-fill-1 hover:text-fg focus-ring-inset",
              align === "start" && "flex-1",
            )}
          >
            <span className="min-w-0 truncate">{s}</span>
          </button>
        </li>
      ))}
    </ul>
  );
});
PromptSuggestions.displayName = "PromptSuggestions";
