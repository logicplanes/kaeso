import { forwardRef, useState, type KeyboardEvent } from "react";
import { Input, type InputProps } from "./input.js";
import { InputGroup } from "./input-group.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_SENSITIVE_INPUT_VARIANTS = {
  kind: {
    secret: { classes: "", description: "Provider keys and tokens: never autocompleted (default)" },
    password: { classes: "", description: "Passwords: browser password managers allowed, Caps Lock hint" },
  },
} as const;
export const KAESO_SENSITIVE_INPUT_DEFAULT_VARIANTS = { kind: "secret" } as const;

export interface SensitiveInputProps extends Omit<InputProps, "type" | "trailing"> {
  kind?: keyof typeof KAESO_SENSITIVE_INPUT_VARIANTS.kind | undefined;
  defaultRevealed?: boolean | undefined;
}

export const SensitiveInput = forwardRef<HTMLInputElement, SensitiveInputProps>(function SensitiveInput(
  { kind = "secret", defaultRevealed = false, onKeyDown, onKeyUp, onBlur, ...props },
  ref,
) {
  const t = useUiText();
  const [revealed, setRevealed] = useState(defaultRevealed);
  const [caps, setCaps] = useState(false);
  const checkCaps = (e: KeyboardEvent<HTMLInputElement>) => setCaps(kind === "password" && e.getModifierState?.("CapsLock"));
  return (
    <Input
      ref={ref}
      type={revealed ? "text" : "password"}
      autoComplete={kind === "password" ? "current-password" : "off"}
      autoCorrect="off"
      autoCapitalize="off"
      spellCheck={false}
      data-1p-ignore={kind === "secret" ? true : undefined}
      data-lpignore={kind === "secret" ? "true" : undefined}
      data-mono={revealed || undefined}
      onBlur={(e) => {
        setCaps(false);
        onBlur?.(e);
      }}
      onKeyDown={(e) => {
        checkCaps(e);
        onKeyDown?.(e);
      }}
      onKeyUp={(e) => {
        checkCaps(e);
        onKeyUp?.(e);
      }}
      trailing={
        <>
          {kind === "password" && (
            <span className="inline-flex shrink-0 items-center gap-4 text-small text-warning-text select-none">
              {caps && <Icon icon={icons.warning} size="sm" />}
              {caps && t("sensitiveInput.capsLockShort", "Caps Lock")}
              <span role="status" className="sr-only">
                {caps ? t("sensitiveInput.capsLock", "Caps Lock is on.") : ""}
              </span>
            </span>
          )}
          <InputGroup.Button
            icon={revealed ? icons.hide : icons.show}
            aria-label={
              kind === "password"
                ? revealed
                  ? t("sensitiveInput.hidePassword", "Hide password")
                  : t("sensitiveInput.showPassword", "Show password")
                : revealed
                  ? t("sensitiveInput.hideKey", "Hide key")
                  : t("sensitiveInput.showKey", "Show key")
            }
            onClick={() => setRevealed((r) => !r)}
            disabled={props.disabled}
          />
        </>
      }
      {...props}
    />
  );
});
SensitiveInput.displayName = "SensitiveInput";
