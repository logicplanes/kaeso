import { forwardRef, Fragment, useSyncExternalStore, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { englishText, useUiText, type UiText } from "../../i18n/context.js";

export const KAESO_KBD_VARIANTS = {
  tone: {
    default: { classes: "bg-fill-1 text-fg-secondary shadow-ring", description: "On panels and raised surfaces" },
    onInk: { classes: "bg-on-ink/15 text-on-ink/80 shadow-none", description: "Inside ink surfaces (toasts, the selection bar)" },
  },
} as const;
export const KAESO_KBD_DEFAULT_VARIANTS = { tone: "default" } as const;

const isMac = () => typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
const subscribePlatform = () => () => undefined;
const useMac = () => useSyncExternalStore(subscribePlatform, isMac, () => false);

function modifiers(t: UiText): Record<string, [mac: string, other: string, name: string]> {
  const ctrl = t("kbd.ctrl", "Ctrl");
  const option = t("kbd.optionName", "Option");
  return {
    mod: ["⌘", ctrl, t("kbd.commandName", "Command")],
    cmd: ["⌘", ctrl, t("kbd.commandName", "Command")],
    ctrl: ["⌃", ctrl, t("kbd.controlName", "Control")],
    alt: ["⌥", "Alt", option],
    option: ["⌥", "Alt", option],
    shift: ["⇧", t("kbd.shift", "Shift"), t("kbd.shiftName", "Shift")],
    enter: ["↵", "Enter", t("kbd.enterName", "Enter")],
    esc: ["esc", "Esc", t("kbd.escapeName", "Escape")],
    backspace: ["⌫", t("kbd.backspace", "Backspace"), t("kbd.backspaceName", "Backspace")],
    up: ["↑", "↑", t("kbd.upName", "Up")],
    down: ["↓", "↓", t("kbd.downName", "Down")],
    left: ["←", "←", t("kbd.leftName", "Left")],
    right: ["→", "→", t("kbd.rightName", "Right")],
    tab: ["⇥", "Tab", t("kbd.tabName", "Tab")],
  };
}

export function keyLabel(key: string, t: UiText = englishText, mac: boolean = isMac()): { visual: string; spoken: string } {
  const m = modifiers(t)[key.toLowerCase()];
  if (!m) return { visual: key.length === 1 ? key.toUpperCase() : key, spoken: key };
  return { visual: mac ? m[0] : m[1], spoken: m[2] };
}

export interface KbdProps extends HTMLAttributes<HTMLElement> {
  children: ReactNode;
  tone?: keyof typeof KAESO_KBD_VARIANTS.tone | undefined;
}

export const Kbd = forwardRef<HTMLElement, KbdProps>(function Kbd({ children, tone = "default", className, ...props }, ref) {
  const t = useUiText();
  const mac = useMac();
  const label = typeof children === "string" ? keyLabel(children, t, mac) : null;
  const spoken = label && label.spoken.toLowerCase() !== label.visual.toLowerCase() ? label.spoken : null;
  return (
    <kbd
      ref={ref}
      className={cn(
        "relative inline-flex h-20 min-w-20 shrink-0 items-center justify-center rounded-kbd px-5 font-sans text-caption font-medium tabular",
        KAESO_KBD_VARIANTS.tone[tone].classes,
        className,
      )}
      {...props}
    >
      {spoken ? (
        <>
          <span aria-hidden="true">{label?.visual}</span>
          <span className="sr-only">{spoken}</span>
        </>
      ) : label ? (
        label.visual
      ) : (
        children
      )}
    </kbd>
  );
});
Kbd.displayName = "Kbd";

export interface KbdSequenceProps extends HTMLAttributes<HTMLSpanElement> {
  keys: string[][] | string[];
  tone?: keyof typeof KAESO_KBD_VARIANTS.tone | undefined;
}

export const KbdSequence = forwardRef<HTMLSpanElement, KbdSequenceProps>(function KbdSequence({ keys, tone = "default", className, ...props }, ref) {
  const t = useUiText();
  const steps: string[][] = keys.length > 0 && Array.isArray(keys[0]) ? (keys as string[][]) : [keys as string[]];
  return (
    <span ref={ref} className={cn("inline-flex items-center gap-4 align-middle", className)} {...props}>
      {steps.map((chord, i) => (
        <Fragment key={i}>
          {i > 0 && <span className={cn("text-caption font-medium", tone === "onInk" ? "text-on-ink/70" : "text-fg-secondary")}>{t("kbd.then", "then")}</span>}
          <span className="inline-flex items-center gap-2">
            {chord.map((k, j) => (
              <Kbd key={`${k}-${j}`} tone={tone}>
                {k}
              </Kbd>
            ))}
          </span>
        </Fragment>
      ))}
    </span>
  );
});
KbdSequence.displayName = "KbdSequence";
