import { forwardRef, useState, type KeyboardEvent } from "react";
import { Input, type InputProps } from "./input.js";
import { InputGroup } from "./input-group.js";
import { icons } from "../../icons/vocabulary.js";
import { cn } from "../../utils/cn.js";
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
  { kind = "secret", defaultRevealed = false, description, onKeyDown, onKeyUp, className, ...props },
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
      className={cn(className, revealed && "mono")}
      description={caps ? t("sensitiveInput.capsLock", "Caps Lock is on.") : description}
      onKeyDown={(e) => {
        checkCaps(e);
        onKeyDown?.(e);
      }}
      onKeyUp={(e) => {
        checkCaps(e);
        onKeyUp?.(e);
      }}
      trailing={
        <InputGroup.Button
          icon={revealed ? icons.hide : icons.show}
          aria-label={
            kind === "password"
              ? revealed
                ? t("sensitiveInput.hidePassword", "Hide password")
                : t("sensitiveInput.showPassword", "Show password")
              : revealed
                ? t("sensitiveInput.hide", "Hide")
                : t("sensitiveInput.show", "Show")
          }
          onClick={() => setRevealed((r) => !r)}
          disabled={props.disabled}
        />
      }
      {...props}
    />
  );
});
SensitiveInput.displayName = "SensitiveInput";
