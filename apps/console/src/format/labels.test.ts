import { afterEach, describe, expect, it } from "vitest";
import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { apply, load, type Locale } from "~/i18n";
import {
  accessLabel,
  DIALECT_IDS,
  dialectLabel,
  ERROR_KIND_IDS,
  errorKindLabel,
  inviteStateLabel,
  joinList,
  keyStateLabel,
  limitDescription,
  limitKindLabel,
  limitUnit,
  periodLabel,
  PROVIDER_IDS,
  providerLabel,
  sessionMethodLabel,
  userStateLabel,
} from "./labels";

const speak = async (l: Locale) => {
  apply(await load(l));
  setFormatLocale(l);
};
afterEach(() => setFormatLocale("en"));

describe("limitDescription", () => {
  it("says a limit as the gateway does in English", () => {
    expect(limitDescription({ kind: "spend_budget", period: "month", amount: "50.00" })).toBe("monthly spend budget of 50 USD");
    expect(limitDescription({ kind: "token_budget", period: "total", amount: "1000000" })).toBe("total token budget of 1000000 tokens");
    expect(limitDescription({ kind: "request_rate", period: "minute", amount: "60" })).toBe("request rate of 60 requests per minute");
    expect(limitDescription({ kind: "spend_budget", period: "day", amount: "0.5" })).toBe("daily spend budget of 0.5 USD");
  });

  it("says every kind and period of limit", () => {
    expect(limitDescription({ kind: "token_rate", period: "minute", amount: "40000" })).toBe("token rate of 40000 tokens per minute");
    expect(limitDescription({ kind: "token_budget", period: "day", amount: "5000" })).toBe("daily token budget of 5000 tokens");
    expect(limitDescription({ kind: "token_budget", period: "month", amount: "5000" })).toBe("monthly token budget of 5000 tokens");
    expect(limitDescription({ kind: "spend_budget", period: "total", amount: "0.123456789" })).toBe("total spend budget of 0.123457 USD");
  });

  it("groups large amounts and keeps the period in German", async () => {
    await speak("de");
    expect(limitDescription({ kind: "token_budget", period: "month", amount: "1000000" })).toBe("Token-Budget von 1.000.000 Token pro Monat");
    expect(limitDescription({ kind: "spend_budget", period: "total", amount: "12.5" })).toBe("Ausgabenbudget von 12,5 USD insgesamt");
    expect(limitDescription({ kind: "request_rate", period: "minute", amount: "1200" })).toBe("Anfragerate von 1.200 Anfragen pro Minute");
    expect(limitDescription({ kind: "token_rate", period: "minute", amount: "60" })).toBe("Token-Rate von 60 Token pro Minute");
  });
});

describe("labels", () => {
  it("name every error kind of a request log", () => {
    expect(ERROR_KIND_IDS.map(errorKindLabel)).toEqual([
      "Invalid request",
      "Authentication",
      "Permission denied",
      "Not found",
      "Request too large",
      "Rate limited",
      "Provider overloaded",
      "Context too long",
      "Content filtered",
      "Unsupported by the target format",
      "Timeout",
      "Billing",
      "Provider error",
      "Stream interrupted",
      "Client disconnected",
      "Gateway shutdown",
    ]);
  });

  it("name every error kind in German", async () => {
    await speak("de");
    expect(ERROR_KIND_IDS.map(errorKindLabel)).toEqual([
      "Ungültige Anfrage",
      "Authentifizierung",
      "Zugriff verweigert",
      "Nicht gefunden",
      "Anfrage zu groß",
      "Ratenlimit erreicht",
      "Anbieter überlastet",
      "Kontext zu lang",
      "Inhalt gefiltert",
      "Vom Zielformat nicht unterstützt",
      "Zeitüberschreitung",
      "Abrechnung",
      "Anbieterfehler",
      "Stream unterbrochen",
      "Client getrennt",
      "Gateway heruntergefahren",
    ]);
  });

  it("write a value the console doesn't know yet as words", () => {
    expect(errorKindLabel("upstream_gone_away")).toBe("Upstream gone away");
    expect(providerLabel("mistral")).toBe("Mistral");
    expect(accessLabel("read_only")).toBe("Read only");
    expect(keyStateLabel("suspended")).toBe("Suspended");
    expect(limitKindLabel("image_budget")).toBe("Image budget");
    expect(limitUnit("image_budget")).toBe("");
    expect(periodLabel("week")).toBe("Week");
    expect(sessionMethodLabel("passkey")).toBe("Passkey");
    expect(userStateLabel("locked")).toBe("Locked");
    expect(inviteStateLabel("bounced")).toBe("Bounced");
    expect(dialectLabel("gemini_generate")).toBe("gemini_generate");
  });

  it("keep product names as they are in every language", async () => {
    await speak("zh-CN");
    expect(DIALECT_IDS.map(dialectLabel)).toEqual(["Anthropic Messages", "OpenAI Chat Completions", "OpenAI Responses", "OpenAI Embeddings"]);
    expect(PROVIDER_IDS.map(providerLabel)).toEqual(["OpenAI", "Anthropic", "DeepSeek", "SpaceX", "Together AI"]);
    expect(sessionMethodLabel("oidc")).toBe("SSO");
    expect(limitUnit("spend_budget")).toBe("USD");
  });

  it("name limits, their units and periods", () => {
    expect(["request_rate", "token_rate", "token_budget", "spend_budget"].map((k) => [limitKindLabel(k), limitUnit(k)])).toEqual([
      ["Request rate", "requests / min"],
      ["Token rate", "tokens / min"],
      ["Token budget", "tokens"],
      ["Spend budget", "USD"],
    ]);
    expect(["minute", "day", "month", "total"].map(periodLabel)).toEqual(["Per minute", "Per day", "Per month", "In total"]);
  });

  it("name limits, their units and periods in German", async () => {
    await speak("de");
    expect(["request_rate", "token_rate", "token_budget", "spend_budget"].map((k) => [limitKindLabel(k), limitUnit(k)])).toEqual([
      ["Anfragerate", "Anfragen / Min."],
      ["Token-Rate", "Token / Min."],
      ["Token-Budget", "Token"],
      ["Ausgabenbudget", "USD"],
    ]);
    expect(["minute", "day", "month", "total"].map(periodLabel)).toEqual(["Pro Minute", "Pro Tag", "Pro Monat", "Insgesamt"]);
  });

  it("name key, user and invite states, access and sign-in methods", () => {
    expect(["inference", "platform", "both"].map(accessLabel)).toEqual(["Inference", "Platform", "Inference + platform"]);
    expect(["active", "disabled", "expired", "archived"].map(keyStateLabel)).toEqual(["Active", "Disabled", "Expired", "Archived"]);
    expect(["active", "deactivated", "deleted"].map(userStateLabel)).toEqual(["Active", "Deactivated", "Deleted"]);
    expect(["pending", "accepted", "revoked", "expired"].map(inviteStateLabel)).toEqual(["Pending", "Accepted", "Revoked", "Expired"]);
    expect(["password", "magic_link", "oidc", "setup", "signup"].map(sessionMethodLabel)).toEqual(["Password", "Sign-in link", "SSO", "Setup", "Sign-up"]);
  });

  it("name key, user and invite states, access and sign-in methods in German", async () => {
    await speak("de");
    expect(["inference", "platform", "both"].map(accessLabel)).toEqual(["Inferenz", "Plattform", "Inferenz + Plattform"]);
    expect(keyStateLabel("disabled")).toBe("Deaktiviert");
    expect(["deactivated", "deleted"].map(userStateLabel)).toEqual(["Deaktiviert", "Gelöscht"]);
    expect(["pending", "accepted"].map(inviteStateLabel)).toEqual(["Ausstehend", "Angenommen"]);
    expect(["password", "magic_link", "setup", "signup"].map(sessionMethodLabel)).toEqual(["Passwort", "Anmeldelink", "Einrichtung", "Registrierung"]);
  });

  it("join a list with the language's separator", async () => {
    expect(joinList(["Ada", "Grace", "Linus"])).toBe("Ada, Grace, Linus");
    expect(joinList([])).toBe("");
    await speak("zh-CN");
    expect(joinList(["Ada", "Grace"])).toBe("Ada、Grace");
  });
});
