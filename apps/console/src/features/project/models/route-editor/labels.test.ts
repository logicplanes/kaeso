import { describe, expect, it } from "vitest";
import type { RouteRule, RouteStep } from "~/api/generated/types";
import { fromRoute, type DraftCondition, type DraftSplit, type Problem } from "./draft";
import {
  conditionSubtitle,
  conditionTitle,
  failureHint,
  failureLabel,
  featureLabel,
  fieldLabel,
  headerHint,
  kindLabel,
  moveLabel,
  problemLabel,
  ruleSummary,
  splitSubtitle,
  splitTitle,
  templateHint,
  templateLabel,
  weightLabel,
} from "./labels";

const names = {
  key: (id: string) => ({ key_web: "Web app", key_ci: "CI", key_cron: "Cron", key_bot: "Bot" })[id] ?? id,
  member: (id: string) => (id === "usr_ada" ? "Ada Okafor" : id),
  serviceAccount: (id: string) => (id === "svc_batch" ? "Nightly batch" : id),
};

const model: RouteStep = { type: "model", model: "m1", fallback: null, fall_back_on: ["unavailable"], timeout_seconds: null };

describe("route labels", () => {
  it("say every rule as a sentence", () => {
    const cases: [RouteRule, string][] = [
      [{ field: "header", name: "x-plan", op: "equals", value: "paid" }, "x-plan is paid"],
      [{ field: "header", name: "x-plan", op: "not_equals", value: "paid" }, "x-plan isn't paid"],
      [{ field: "header", name: "x-plan", op: "contains", value: "pa" }, "x-plan contains pa"],
      [{ field: "header", name: "x-beta", op: "present", value: null }, "x-beta is sent"],
      [{ field: "header", name: "x-beta", op: "absent" }, "x-beta isn't sent"],
      [{ field: "header", name: " ", op: "present", value: null }, "A header is sent"],
      [{ field: "format", op: "in", value: ["anthropic_messages"] }, "Sent as Anthropic Messages"],
      [{ field: "format", op: "not_in", value: ["openai_chat", "openai_responses"] }, "Not sent as OpenAI Chat Completions or OpenAI Responses"],
      [{ field: "feature", op: "used", value: "images" }, "Uses Images"],
      [{ field: "feature", op: "not_used", value: "tools" }, "Doesn't use Tools"],
      [{ field: "input_tokens", op: "at_least", value: 100000 }, "100,000 input tokens or more"],
      [{ field: "input_tokens", op: "below", value: 2000 }, "Under 2,000 input tokens"],
      [{ field: "key", op: "in", value: ["key_web"] }, "Sent with Web app"],
      [{ field: "key", op: "not_in", value: ["key_web", "key_ci", "key_cron", "key_bot"] }, "Not sent with Web app or 3 others"],
      [{ field: "member", op: "in", value: ["usr_ada"] }, "Sent by Ada Okafor"],
      [{ field: "member", op: "not_in", value: ["usr_ada"] }, "Not sent by Ada Okafor"],
      [{ field: "service_account", op: "in", value: ["svc_batch"] }, "Sent by Nightly batch"],
      [{ field: "service_account", op: "not_in", value: ["svc_batch"] }, "Not sent by Nightly batch"],
    ];
    for (const [rule, text] of cases) expect(ruleSummary(rule, names)).toBe(text);
  });

  it("name a condition by its first rule and count the rest", () => {
    const condition = (rules: RouteRule[], match: "all" | "any") => fromRoute({ type: "condition", match, rules, then: model, else: model }) as DraftCondition;
    const header: RouteRule = { field: "header", name: "x-plan", op: "equals", value: "paid" };
    const tokens: RouteRule = { field: "input_tokens", op: "below", value: 10 };
    expect(conditionTitle(condition([header, tokens], "all"), names)).toBe("x-plan is paid");
    expect(conditionSubtitle(condition([header], "all"))).toBe("1 rule");
    expect(conditionSubtitle(condition([header, tokens], "all"))).toBe("All of 2 rules");
    expect(conditionSubtitle(condition([header, tokens], "any"))).toBe("Any of 2 rules");
    expect(conditionTitle({ ...condition([header], "all"), rules: [] }, names)).toBe("No rules yet");
  });

  it("name a split by how it draws", () => {
    const split = (by: "request" | "caller") =>
      fromRoute({
        type: "split",
        by,
        branches: [
          { weight: 50, step: model },
          { weight: 50, step: model },
        ],
      }) as DraftSplit;
    expect(splitTitle(split("caller"))).toBe("By caller");
    expect(splitTitle(split("request"))).toBe("Per request");
    expect(splitTitle({ ...split("request"), by: "header", header: " x-user-id " })).toBe("By x-user-id");
    expect(splitTitle({ ...split("request"), by: "header", header: "" })).toBe("By header");
    expect(splitSubtitle(split("request"))).toBe("2 branches");
    expect(weightLabel(80)).toBe("80%");
  });

  it("say every problem in a few words", () => {
    const problems: [Problem, string][] = [
      [{ kind: "noModel" }, "Choose a model"],
      [{ kind: "removedModel" }, "Model removed"],
      [{ kind: "weights", total: 90 }, "Weights add up to 90%"],
      [{ kind: "headerName" }, "Name the header"],
      [{ kind: "secretHeader" }, "Header carries credentials"],
      [{ kind: "headerValue" }, "Give the header's value"],
      [{ kind: "noFormats" }, "Choose a format"],
      [{ kind: "tokens" }, "Give a token count"],
      [{ kind: "noIds" }, "Choose at least one"],
      [{ kind: "timeout" }, "Timeout out of range"],
      [{ kind: "fallBackOn" }, "Choose when it falls back"],
      [{ kind: "tooMany" }, "Over 32 steps"],
      [{ kind: "tooDeep" }, "Over 12 steps deep"],
    ];
    for (const [p, text] of problems) expect(problemLabel(p)).toBe(text);
  });

  it("name kinds, failures, features and fields", () => {
    expect(Object.values(kindLabel).map((f) => f())).toEqual(["Start", "Model", "Condition", "Split"]);
    expect(Object.values(failureLabel).every((f) => f().length > 0)).toBe(true);
    expect(Object.values(failureHint).every((f) => f().endsWith("."))).toBe(true);
    expect(Object.values(featureLabel).map((f) => f())).toEqual(["Images", "PDFs", "Tools", "Reasoning", "A JSON schema", "Streaming"]);
    expect(Object.values(fieldLabel).map((f) => f())).toEqual(["Header", "API format", "Feature", "Input tokens", "API key", "Member", "Service account"]);
  });

  it("name every template and every move", () => {
    expect(Object.values(templateLabel).map((f) => f())).toEqual(["One model", "A fallback chain", "By a header", "Long prompts elsewhere", "An A/B test"]);
    expect(Object.values(templateHint).every((f) => f().endsWith("."))).toBe(true);
    expect(Object.values(moveLabel).map((f) => f())).toEqual(["Try earlier", "Try later"]);
    expect(headerHint({ kind: "headerName" })).toBe("Enter a lowercase header name, such as x-user-id.");
    expect(headerHint({ kind: "secretHeader" })).toBe("Routes can't read headers that carry credentials.");
  });
});
