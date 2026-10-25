import { useState, type ReactNode } from "react";
import { SwapText } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { isApiError } from "~/api/errors";
import { t } from "~/i18n";

export interface FormError {
  message: string;
  param: string | null;
  type: string | null;
}

export function useSubmit() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<FormError | null>(null);
  const submit = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      const failure = isApiError(e)
        ? { message: e.message, param: e.param, type: e.type }
        : { message: t("common.unreachable", "Can't reach the gateway; try again."), param: null, type: null };
      setError(failure);
      toast.error(failure.message);
    } finally {
      setBusy(false);
    }
  };
  return {
    busy,
    error,
    setError,
    submit,
  };
}

function passwordStatus(password: string): string {
  if (password.length === 0) return t("public.passwordRule", "12 to 128 characters. A few unrelated words are easy to remember and hard to guess.");
  if (password.length < 12)
    return t("public.passwordShort", "{count, plural, one {# more character to go.} other {# more characters to go.}}", { count: 12 - password.length });
  if (password.length > 128) return t("public.passwordLong", "At most 128 characters.");
  return t("public.passwordOk", "Long enough.");
}

export function passwordHint(password: string, empty = passwordStatus("")): ReactNode {
  return (
    <SwapText
      align="start"
      reserve={[...new Set([empty, passwordStatus(""), passwordStatus("x".repeat(11)), passwordStatus("x".repeat(129)), passwordStatus("x".repeat(12))])]}
    >
      {password ? passwordStatus(password) : empty}
    </SwapText>
  );
}

export const passwordOk = (p: string) => p.length >= 12 && p.length <= 128;
