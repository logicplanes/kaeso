import { forwardRef } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon, type IconData } from "../icon/icon.js";
import { MarkGlyph } from "./identity.js";

/** The tile is a provider tile, in the same four sizes. */
export const KAESO_SOURCE_BADGE_VARIANTS = {
  size: {
    sm: { classes: "size-16 rounded-cell", description: "16px, inside table cells" },
    md: { classes: "size-20 rounded-avatar", description: "20px (default)" },
    lg: { classes: "size-28 rounded-org", description: "28px" },
    xl: { classes: "size-40 rounded-org", description: "40px: a source's own page" },
  },
} as const;
export const KAESO_SOURCE_BADGE_DEFAULT_VARIANTS = { size: "md" } as const;

/**
 * The marks of the clients and gateways that report to Kaeso as sources, on a 24px grid, drawn
 * in the text colour. OpenRouter's is its provider mark; LiteLLM publishes no vector mark and
 * HTTP is no product, so both show an icon.
 */
export const SOURCE_MARKS: Readonly<Record<string, string>> = {
  claude_code:
    "M20.998 10.949H24v3.102h-3v3.028h-1.487V20H18v-2.921h-1.487V20H15v-2.921H9V20H7.488v-2.921H6V20H4.487v-2.921H3V14.05H0V10.95h3V5h17.998v5.949zM6 10.949h1.488V8.102H6v2.847zm10.51 0H18V8.102h-1.49v2.847z",
  codex:
    "M8.086.457a6.105 6.105 0 013.046-.415c1.333.153 2.521.72 3.564 1.7a.117.117 0 00.107.029c1.408-.346 2.762-.224 4.061.366l.063.03.154.076c1.357.703 2.33 1.77 2.918 3.198.278.679.418 1.388.421 2.126a5.655 5.655 0 01-.18 1.631.167.167 0 00.04.155 5.982 5.982 0 011.578 2.891c.385 1.901-.01 3.615-1.183 5.14l-.182.22a6.063 6.063 0 01-2.934 1.851.162.162 0 00-.108.102c-.255.736-.511 1.364-.987 1.992-1.199 1.582-2.962 2.462-4.948 2.451-1.583-.008-2.986-.587-4.21-1.736a.145.145 0 00-.14-.032c-.518.167-1.04.191-1.604.185a5.924 5.924 0 01-2.595-.622 6.058 6.058 0 01-2.146-1.781c-.203-.269-.404-.522-.551-.821a7.74 7.74 0 01-.495-1.283 6.11 6.11 0 01-.017-3.064.166.166 0 00.008-.074.115.115 0 00-.037-.064 5.958 5.958 0 01-1.38-2.202 5.196 5.196 0 01-.333-1.589 6.915 6.915 0 01.188-2.132c.45-1.484 1.309-2.648 2.577-3.493.282-.188.55-.334.802-.438.286-.12.573-.22.861-.304a.129.129 0 00.087-.087A6.016 6.016 0 015.635 2.31C6.315 1.464 7.132.846 8.086.457zm-.804 7.85a.848.848 0 00-1.473.842l1.694 2.965-1.688 2.848a.849.849 0 001.46.864l1.94-3.272a.849.849 0 00.007-.854l-1.94-3.393zm5.446 6.24a.849.849 0 000 1.695h4.848a.849.849 0 000-1.696h-4.848z",
  gemini_cli:
    "M16.793 10.358v3.867L7.236 18.82v-2.8l7.751-3.728-7.75-3.728V5.763l9.556 4.595z M19.608 0A4.392 4.392 0 0124 4.392v15.216A4.392 4.392 0 0119.608 24H4.392A4.392 4.392 0 010 19.608V4.392A4.392 4.392 0 014.392 0h15.216zM4.26 1.444A2.816 2.816 0 001.444 4.26v15.48a2.816 2.816 0 002.816 2.816h15.48a2.816 2.816 0 002.816-2.816V4.26a2.816 2.816 0 00-2.816-2.816H4.26z",
  antigravity:
    "M21.751 22.607c1.34 1.005 3.35.335 1.508-1.508C17.73 15.74 18.904 1 12.037 1 5.17 1 6.342 15.74.815 21.1c-2.01 2.009.167 2.511 1.507 1.506 5.192-3.517 4.857-9.714 9.715-9.714 4.857 0 4.522 6.197 9.714 9.715z",
  opentelemetry:
    "M12.6974 13.1173c-1.0224 1.0224-1.0224 2.68 0 3.7024 1.0224 1.0224 2.68 1.0224 3.7024 0 1.0224-1.0223 1.0224-2.68 0-3.7024-1.0223-1.0223-2.68-1.0223-3.7024 0zm2.7677 2.7701c-.5063.5063-1.3267.5063-1.833 0s-.5063-1.3266 0-1.833c.5063-.5062 1.3267-.5062 1.833 0 .5063.504.5063 1.3267 0 1.833zM16.356.2355l-1.6041 1.6042c-.314.314-.314.83 0 1.144L21.015 9.247c.314.314.83.314 1.144 0l1.6042-1.6041c.314-.314.314-.83 0-1.144L17.4976.2354c-.314-.314-.8276-.314-1.1416 0zM5.1173 20.734c.2848-.2848.2848-.7497 0-1.0345l-.8155-.8155c-.2848-.2848-.7497-.2848-1.0345 0l-1.6845 1.6845-.0024.0024-.4625-.4625c-.2556-.2556-.6718-.2556-.925 0-.2556.2556-.2556.6718 0 .925l2.775 2.775c.2556.2556.6718.2556.925 0 .2532-.2556.2556-.6718 0-.925l-.4625-.4625.0024-.0024zm8.4856-15.893-3.5637 3.5637c-.3164.3164-.3164.8374 0 1.1538l2.2006 2.2005c1.5554-1.1197 3.7365-.981 5.1361.4187l1.7819-1.7818c.3164-.3165.3164-.8374 0-1.1538l-4.401-4.401c-.3165-.319-.8374-.319-1.1539 0zm-2.2881 7.8455-1.2999-1.2999c-.3043-.3043-.8033-.3043-1.1076 0l-4.5836 4.586c-.3042.3043-.3042.8033 0 1.1076l2.5973 2.5973c.3043.3043.8033.3043 1.1076 0l2.9478-2.9527c-.6231-1.2877-.5112-2.8431.3384-4.0383z",
  openrouter:
    "M18.376 3.806c2.693 0 4.876 2.201 4.876 4.917s-2.183 4.916-4.876 4.916l4.836 4.876c.615.62.18 1.679-.689 1.679H8.626C4.138 20.194.5 16.525.5 12s3.638-8.194 8.126-8.194zm-9.75 3.278c-2.693 0-4.876 2.2-4.876 4.916 0 2.715 2.183 4.916 4.876 4.916 2.692 0 4.875-2.2 4.875-4.916 0-2.715-2.183-4.916-4.875-4.916",
};

const SOURCE_ICONS: Readonly<Record<string, IconData>> = {
  litellm: icons.gateway,
  http: icons.api,
};

/** Each kind's product name, which no language translates. */
export const SOURCE_NAMES: Readonly<Record<string, string>> = {
  claude_code: "Claude Code",
  codex: "Codex",
  gemini_cli: "Gemini CLI",
  antigravity: "Antigravity",
  opentelemetry: "OpenTelemetry",
  http: "HTTP",
  litellm: "LiteLLM",
  openrouter: "OpenRouter",
};

const MARK_SIZE = { sm: "size-10", md: "size-12", lg: "size-16", xl: "size-24" } as const;

export interface SourceBadgeProps {
  /** The source's kind, such as `claude_code`; a kind without a mark shows the source icon. */
  kind: string;
  /** The name beside the tile, or the tile's accessible name; the kind's product name by default. */
  name?: string | undefined;
  size?: keyof typeof KAESO_SOURCE_BADGE_VARIANTS.size | undefined;
  /** Shows the name beside the tile. */
  withName?: boolean | undefined;
  /** Hides the tile from assistive technology, where the text beside it already names the client. */
  decorative?: boolean | undefined;
  className?: string | undefined;
}

/** A source's client as a tile the size of a provider's, with its mark in the text colour, and its name beside it. */
export const SourceBadge = forwardRef<HTMLSpanElement, SourceBadgeProps>(function SourceBadge(
  { kind, name, size = "md", withName, decorative, className },
  ref,
) {
  const label = name?.trim() || SOURCE_NAMES[kind] || kind;
  const mark = SOURCE_MARKS[kind];
  const tile = (
    <span
      aria-hidden={withName || decorative || undefined}
      {...(!withName && !decorative ? { role: "img", "aria-label": label } : {})}
      data-source={kind}
      className={cn("inline-flex shrink-0 items-center justify-center bg-raised text-fg shadow-ring", KAESO_SOURCE_BADGE_VARIANTS.size[size].classes)}
    >
      {mark ? (
        <MarkGlyph mark={{ d: mark, color: "text-fg" }} className={MARK_SIZE[size]} />
      ) : (
        <Icon icon={SOURCE_ICONS[kind] ?? icons.source} className={MARK_SIZE[size]} />
      )}
    </span>
  );
  if (!withName)
    return (
      <span ref={ref} className={cn("inline-flex", className)}>
        {tile}
      </span>
    );
  return (
    <span ref={ref} className={cn("inline-flex max-w-full min-w-0 items-center gap-8 text-body text-fg", className)}>
      {tile}
      <span className="min-w-0 truncate">{label}</span>
    </span>
  );
});
SourceBadge.displayName = "SourceBadge";
