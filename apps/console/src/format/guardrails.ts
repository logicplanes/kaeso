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

/** How many values of one kind built-in rules found, such as "2 API keys". */
export function foundLabel(kind: string, count: number): string {
  switch (kind) {
    case "api_key":
      return t("format.guardrail.found.apiKey", "{count, plural, one {# API key} other {# API keys}}", { count });
    case "private_key":
      return t("format.guardrail.found.privateKey", "{count, plural, one {# private key} other {# private keys}}", { count });
    case "token":
      return t("format.guardrail.found.token", "{count, plural, one {# JSON Web Token} other {# JSON Web Tokens}}", { count });
    case "email":
      return t("format.guardrail.found.email", "{count, plural, one {# email address} other {# email addresses}}", { count });
    case "phone":
      return t("format.guardrail.found.phone", "{count, plural, one {# phone number} other {# phone numbers}}", { count });
    case "iban":
      return t("format.guardrail.found.iban", "{count, plural, one {# IBAN} other {# IBANs}}", { count });
    case "card":
      return t("format.guardrail.found.card", "{count, plural, one {# card number} other {# card numbers}}", { count });
    case "ip":
      return t("format.guardrail.found.ip", "{count, plural, one {# IP address} other {# IP addresses}}", { count });
    default:
      return t("format.guardrail.found.custom", "{count, plural, one {# match} other {# matches}} of {name}", { count, name: kind });
  }
}
