import type { GuardrailAction, GuardrailFailure, GuardrailPhase } from "~/api/generated/types";
import { t } from "~/i18n";

export function guardrailActionLabel(a: GuardrailAction): string {
  if (a === "block") return t("format.guardrail.block", "Blocked");
  if (a === "replace") return t("format.guardrail.replace", "Rewritten");
  return t("format.guardrail.allow", "Allowed");
}

export function guardrailPhaseLabel(p: GuardrailPhase): string {
  return p === "reply" ? t("format.guardrail.reply", "Reply") : t("format.guardrail.request", "Request");
}

export function guardrailFailureLabel(f: GuardrailFailure): string {
  switch (f) {
    case "timeout":
      return t("format.guardrail.timeout", "No answer in time");
    case "unreachable":
      return t("format.guardrail.unreachable", "Can't be reached");
    case "refused":
      return t("format.guardrail.refused", "Address not allowed");
    case "status":
      return t("format.guardrail.status", "Answered with an error");
    default:
      return t("format.guardrail.invalid", "No verdict in the answer");
  }
}
