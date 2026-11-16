import { useState } from "react";
import { Button, Input, KeyValue, Select, Switch, Text, Textarea, icons } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { GuardrailAction, GuardrailPattern as CustomRule, GuardrailRuleAction as RuleAction, GuardrailRules as Rules } from "~/api/generated/types";

type Detector = "secrets" | "email" | "phone" | "iban" | "card" | "ip";
import { useApiMutation } from "~/auth/mutation";
import { foundLabel, guardrailActionLabel } from "~/format/guardrails";
import { t } from "~/i18n";

const DETECTORS: Detector[] = ["secrets", "email", "phone", "iban", "card", "ip"];
const MAX_CUSTOM = 20;

function detectorLabel(d: Detector): string {
  switch (d) {
    case "secrets":
      return t("org.guardrails.detector.secrets", "Secrets and API keys");
    case "email":
      return t("org.guardrails.detector.email", "Email addresses");
    case "phone":
      return t("org.guardrails.detector.phone", "Phone numbers");
    case "iban":
      return t("org.guardrails.detector.iban", "IBANs");
    case "card":
      return t("org.guardrails.detector.card", "Payment card numbers");
    default:
      return t("org.guardrails.detector.ip", "IP addresses");
  }
}

const actionOptions = () => [
  { value: "mask", label: t("org.guardrails.rule.mask", "Mask") },
  { value: "block", label: t("org.guardrails.rule.block", "Block") },
  { value: "flag", label: t("org.guardrails.rule.flag", "Count only") },
];

/** Rules with every field, as the editor works on them. */
export type FullRules = Required<Rules>;

export const fullRules = (r?: Rules | null): FullRules => ({
  detectors: r?.detectors ?? { secrets: "mask" },
  custom: r?.custom ?? [],
  mask_log: r?.mask_log ?? false,
  restore: r?.restore ?? false,
});

/** Rules a person can save: at least one detector or a named pattern. */
export function rulesProblem(r: FullRules): string | undefined {
  const detectors = Object.keys(r.detectors).length;
  const custom = r.custom;
  if (detectors === 0 && custom.length === 0) return t("org.guardrails.chooseRule", "Turn on a detector or add a pattern");
  if (custom.some((c) => !c.name.trim() || !c.pattern)) return t("org.guardrails.completePatterns", "Give every pattern a name and an expression");
  return undefined;
}

export function RulesFields({ org, rules, set }: { org: string; rules: FullRules; set: (r: FullRules) => void }) {
  const detectors = rules.detectors;
  const custom = rules.custom;
  const setDetector = (d: Detector, v: string) => {
    const others = Object.entries(detectors).filter(([k]) => k !== d);
    set({ ...rules, detectors: Object.fromEntries(v === "off" ? others : [...others, [d, v as RuleAction]]) });
  };
  const setCustom = (i: number, c: Partial<CustomRule>) => set({ ...rules, custom: custom.map((x, j) => (j === i ? { ...x, ...c } : x)) });
  return (
    <div className="flex flex-col gap-16">
      <div className="flex flex-col gap-8">
        {DETECTORS.map((d) => (
          <div key={d} className="flex items-center justify-between gap-16">
            <Text>{detectorLabel(d)}</Text>
            <div className="w-160">
              <Select
                aria-label={detectorLabel(d)}
                size="sm"
                fullWidth
                value={detectors[d] ?? "off"}
                onValueChange={(v) => setDetector(d, v)}
                options={[{ value: "off", label: t("org.guardrails.rule.off", "Off") }, ...actionOptions()]}
              />
            </div>
          </div>
        ))}
      </div>
      {custom.map((c, i) => (
        <div key={i} className="flex items-end gap-8">
          <Input
            label={t("org.guardrails.patternName", "Pattern name")}
            placeholder={t("org.guardrails.patternNameExample", "order_id")}
            value={c.name}
            onChange={(e) => setCustom(i, { name: e.target.value })}
            maxLength={32}
          />
          <Input
            label={t("org.guardrails.pattern", "Regular expression")}
            placeholder={t("org.guardrails.patternExample", "ORD-\\d{6}")}
            value={c.pattern}
            onChange={(e) => setCustom(i, { pattern: e.target.value })}
            maxLength={500}
          />
          <div className="w-160 shrink-0">
            <Select
              aria-label={t("org.guardrails.patternAction", "What {name} does", { name: c.name || String(i + 1) })}
              size="sm"
              fullWidth
              value={c.action}
              onValueChange={(v) => setCustom(i, { action: v as RuleAction })}
              options={actionOptions()}
            />
          </div>
          <Button
            variant="ghost"
            size="sm"
            icon={icons.delete}
            aria-label={t("org.guardrails.removePattern", "Remove pattern {n}", { n: i + 1 })}
            onClick={() => set({ ...rules, custom: custom.filter((_, j) => j !== i) })}
          />
        </div>
      ))}
      <div>
        <Button
          size="sm"
          leadingIcon={icons.add}
          disabled={custom.length >= MAX_CUSTOM}
          onClick={() => set({ ...rules, custom: [...custom, { name: "", pattern: "", action: "mask" }] })}
        >
          {t("org.guardrails.addPattern", "Add pattern")}
        </Button>
      </div>
      <Switch
        label={t("org.guardrails.maskLog", "Mask in the request log")}
        description={t("org.guardrails.maskLogHint", "The log keeps placeholders, also for values only counted.")}
        checked={!!rules.mask_log}
        onCheckedChange={(on) => set({ ...rules, mask_log: on })}
      />
      <Switch
        label={t("org.guardrails.restore", "Restore in the reply")}
        description={t("org.guardrails.restoreHint", "The app gets the original values where the model wrote placeholders.")}
        checked={!!rules.restore}
        onCheckedChange={(on) => set({ ...rules, restore: on })}
      />
      <TrySample org={org} rules={rules} />
    </div>
  );
}

function TrySample({ org, rules }: { org: string; rules: FullRules }) {
  const [text, setText] = useState("");
  const run = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/orgs/{org}/guardrails/try", { params: { path: { org } }, body: { rules, text } })),
  });
  const result = run.data;
  const masked = result?.messages?.[0]?.content[0];
  return (
    <div className="flex flex-col gap-8">
      <Textarea
        label={t("org.guardrails.sample", "Sample text")}
        placeholder={t("org.guardrails.samplePlaceholder", "Paste a prompt to see what the rules find")}
        value={text}
        onChange={(e) => setText(e.target.value)}
      />
      <div>
        <Button size="sm" loading={run.isPending} disabled={!text.trim() || !!rulesProblem(rules)} onClick={() => run.mutate()}>
          {t("org.guardrails.trySample", "Try rules")}
        </Button>
      </div>
      {result && (
        <KeyValue
          items={[
            {
              label: t("org.guardrails.verdict", "Verdict"),
              value: <Text>{guardrailActionLabel(result.action as GuardrailAction)}</Text>,
            },
            ...(result.message ? [{ label: t("org.guardrails.reason", "Reason"), value: <Text>{result.message}</Text> }] : []),
            ...(masked?.type === "text" ? [{ label: t("org.guardrails.rewritten", "Rewritten text"), value: <Text>{masked.text}</Text>, wrap: true }] : []),
            {
              label: t("org.guardrails.found", "Found"),
              value: (
                <Text>
                  {Object.keys(result.found).length
                    ? Object.entries(result.found)
                        .map(([k, n]) => foundLabel(k, n))
                        .join(t("orgHome.listSeparator", ", "))
                    : t("org.guardrails.foundNothing", "Nothing")}
                </Text>
              ),
            },
          ]}
        />
      )}
    </div>
  );
}
