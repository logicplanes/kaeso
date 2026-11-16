import { useState } from "react";
import { Button, Dialog, KeyValue, Segmented, Text, Textarea } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { GuardrailMessage, GuardrailPhase, GuardrailTestResult, GuardrailView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { t } from "~/i18n";
import { foundLabel, guardrailActionLabel, guardrailFailureLabel } from "~/format/guardrails";

const said = (role: string, text: string): GuardrailMessage => ({ role, content: [{ type: "text", text }] });

const replacementText = (messages: GuardrailMessage[]) =>
  messages
    .flatMap((m) => m.content)
    .flatMap((p) => (p.type === "text" ? [p.text] : []))
    .join("\n");

function Outcome({ result }: { result: GuardrailTestResult }) {
  const rows = [
    {
      label: t("org.guardrails.verdict", "Verdict"),
      value: result.action ? guardrailActionLabel(result.action) : t("org.guardrails.noVerdict", "No verdict"),
    },
    ...(result.category ? [{ label: t("org.guardrails.category", "Category"), value: result.category }] : []),
    ...(result.message ? [{ label: t("org.guardrails.reason", "Reason"), value: result.message }] : []),
    ...(result.messages ? [{ label: t("org.guardrails.rewritten", "Rewritten text"), value: replacementText(result.messages) }] : []),
    ...(result.failure
      ? [
          {
            label: t("org.guardrails.failure", "Failure"),
            value: result.status ? `${guardrailFailureLabel(result.failure)} (${result.status})` : guardrailFailureLabel(result.failure),
          },
        ]
      : []),
    ...(Object.keys(result.found).length
      ? [
          {
            label: t("org.guardrails.found", "Found"),
            value: Object.entries(result.found)
              .map(([k, n]) => foundLabel(k, n))
              .join(t("orgHome.listSeparator", ", ")),
          },
        ]
      : []),
    { label: t("org.guardrails.took", "Took"), value: t("org.guardrails.ms", "{count} ms", { count: result.duration_ms }) },
  ];
  return <KeyValue items={rows.map((r) => ({ label: r.label, value: <Text>{r.value}</Text>, wrap: true }))} />;
}

export function GuardrailTestDialog({ org, guardrail, onClose }: { org: string; guardrail: GuardrailView; onClose: () => void }) {
  const [phase, setPhase] = useState<GuardrailPhase>(guardrail.phases[0] as GuardrailPhase);
  const [prompt, setPrompt] = useState("");
  const [reply, setReply] = useState("");
  const run = useApiMutation({
    mutationFn: async () =>
      data(
        await api.POST("/platform/v1/orgs/{org}/guardrails/{guardrail}/test", {
          params: { path: { org, guardrail: guardrail.id } },
          body: {
            phase,
            messages: [said("user", prompt)],
            ...(phase === "reply" ? { reply: said("assistant", reply) } : {}),
          },
        }),
      ),
  });
  const blocked = !prompt.trim() || (phase === "reply" && !reply.trim()) ? t("org.guardrails.enterSample", "Enter a sample to check") : undefined;
  return (
    <Dialog
      open
      size="lg"
      busy={run.isPending}
      onOpenChange={(o) => !o && onClose()}
      title={t("org.guardrails.testTitle", "Test {name}", { name: guardrail.name })}
      description={t("org.guardrails.testHint", "Kaeso sends the sample as a signed request; nothing of it is stored.")}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.close", "Close")}</Button>
          </Dialog.Close>
          <Button variant="primary" loading={run.isPending} disabled={!!blocked} {...(blocked ? { disabledReason: blocked } : {})} onClick={() => run.mutate()}>
            {t("org.guardrails.runTest", "Send sample")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-16">
        {guardrail.phases.length > 1 && (
          <Segmented
            aria-label={t("org.guardrails.phases", "Checks")}
            value={phase}
            onValueChange={(v) => setPhase(v as GuardrailPhase)}
            options={[
              { value: "request", label: t("org.guardrails.phase.request", "Requests") },
              { value: "reply", label: t("org.guardrails.phase.reply", "Replies") },
            ]}
          />
        )}
        <Textarea label={t("org.guardrails.samplePrompt", "Prompt")} value={prompt} onChange={(e) => setPrompt(e.target.value)} autoFocus />
        {phase === "reply" && <Textarea label={t("org.guardrails.sampleReply", "Reply")} value={reply} onChange={(e) => setReply(e.target.value)} />}
        {run.data && <Outcome result={run.data} />}
      </div>
    </Dialog>
  );
}
