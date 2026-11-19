import { useState } from "react";
import {
  AlertDialog,
  Button,
  Checkbox,
  CheckboxGroup,
  Dialog,
  Input,
  NumberInput,
  Radio,
  RadioGroup,
  SecretReveal,
  StepDialog,
  type StepDialogResult,
} from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { fieldError } from "~/api/field-error";
import type { GuardrailKind, GuardrailOnError, GuardrailPhase, GuardrailStreams, GuardrailView, GuardrailWithSecret } from "~/api/generated/types";
import { guardrailsKey } from "~/api/queries/guardrails";
import { useApiMutation } from "~/auth/mutation";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { toast } from "~/shell/toast";
import { RulesFields, fullRules, rulesProblem, type FullRules } from "./guardrail-rules";

interface Draft {
  kind: GuardrailKind;
  rules: FullRules;
  name: string;
  url: string;
  phases: GuardrailPhase[];
  streams: GuardrailStreams;
  timeout: string;
  onError: GuardrailOnError;
}

const draftOf = (g?: GuardrailView): Draft => ({
  kind: g?.kind ?? "http",
  rules: fullRules(g?.rules),
  name: g?.name ?? "",
  url: g?.url ?? "",
  phases: g?.phases ?? ["request"],
  streams: g?.streams ?? "skip",
  timeout: String(g?.timeout_ms ?? 2000),
  onError: g?.on_error ?? "block",
});

function timeoutProblem(raw: string): string | undefined {
  const ms = Number(raw);
  return Number.isInteger(ms) && ms >= 100 && ms <= 10000 ? undefined : t("org.guardrails.timeoutRange", "Choose 100 to 10000 milliseconds");
}

function WhereFields({ draft, set, error }: { draft: Draft; set: (d: Partial<Draft>) => void; error: Parameters<typeof fieldError>[0] }) {
  return (
    <div className="flex flex-col gap-16">
      <Input
        label={common.name()}
        placeholder={t("org.guardrails.namePlaceholder", "PII check")}
        value={draft.name}
        onChange={(e) => set({ name: e.target.value })}
        error={fieldError(error, "name")}
        maxLength={100}
        autoFocus
      />
      <Input
        label={t("org.guardrails.url", "Address")}
        placeholder={t("org.guardrails.urlPlaceholder", "https://guardrail.example.com/check")}
        value={draft.url}
        onChange={(e) => set({ url: e.target.value })}
        error={fieldError(error, "url")}
        maxLength={2048}
      />
    </div>
  );
}

function HowFields({ draft, set, error }: { draft: Draft; set: (d: Partial<Draft>) => void; error: Parameters<typeof fieldError>[0] }) {
  return (
    <div className="flex flex-col gap-16">
      <CheckboxGroup aria-label={t("org.guardrails.phases", "Checks")} value={draft.phases} onValueChange={(v) => set({ phases: v as GuardrailPhase[] })}>
        <Checkbox
          value="request"
          label={t("org.guardrails.phase.request", "Requests")}
          description={t("org.guardrails.phase.requestHint", "Before a request goes to a model.")}
        />
        <Checkbox
          value="reply"
          label={t("org.guardrails.phase.reply", "Replies")}
          description={t("org.guardrails.phase.replyHint", "Before the model's reply goes back to the app.")}
        />
      </CheckboxGroup>
      {draft.phases.includes("reply") && (
        <RadioGroup
          aria-label={t("org.guardrails.streams", "Streamed replies")}
          value={draft.streams}
          onValueChange={(v) => set({ streams: v as GuardrailStreams })}
        >
          <Radio value="skip" label={t("org.guardrails.streams.skip", "Stream unchecked")} />
          <Radio
            value="hold"
            label={t("org.guardrails.streams.hold", "Hold and check")}
            description={t("org.guardrails.streams.holdHint", "The app gets the first token once the whole reply is checked.")}
          />
        </RadioGroup>
      )}
      <NumberInput
        label={t("org.guardrails.timeout", "Timeout")}
        unit="ms"
        integer
        step="100"
        value={draft.timeout}
        onValueChange={(v) => set({ timeout: v })}
        error={fieldError(error, "timeout_ms") ?? timeoutProblem(draft.timeout)}
      />
      <RadioGroup
        aria-label={t("org.guardrails.onError", "When it fails")}
        value={draft.onError}
        onValueChange={(v) => set({ onError: v as GuardrailOnError })}
      >
        <Radio
          value="block"
          label={t("org.guardrails.onError.block", "Refuse the request")}
          description={t("org.guardrails.onError.blockHint", "Apps get a 503 while the guardrail can't answer.")}
        />
        <Radio
          value="allow"
          label={t("org.guardrails.onError.allow", "Let it through")}
          description={t("org.guardrails.onError.allowHint", "The request log records the failure.")}
        />
      </RadioGroup>
    </div>
  );
}

const body = (d: Draft) =>
  d.kind === "rules"
    ? { name: d.name.trim(), rules: d.rules }
    : {
        name: d.name.trim(),
        url: d.url.trim(),
        phases: d.phases,
        streams: d.streams,
        timeout_ms: Number(d.timeout),
        on_error: d.onError,
      };

function NameField({ draft, set, error }: { draft: Draft; set: (d: Partial<Draft>) => void; error: Parameters<typeof fieldError>[0] }) {
  return (
    <Input
      label={common.name()}
      placeholder={t("org.guardrails.namePlaceholder", "PII check")}
      value={draft.name}
      onChange={(e) => set({ name: e.target.value })}
      error={fieldError(error, "name")}
      maxLength={100}
      autoFocus
    />
  );
}

export function GuardrailDialog({ org, guardrail, onClose }: { org: string; guardrail?: GuardrailView; onClose: () => void }) {
  return guardrail ? <EditGuardrailDialog org={org} guardrail={guardrail} onClose={onClose} /> : <AddGuardrailDialog org={org} onClose={onClose} />;
}

function AddGuardrailDialog({ org, onClose }: { org: string; onClose: () => void }) {
  const [draft, setDraft] = useState(draftOf());
  const [created, setCreated] = useState<GuardrailWithSecret | null>(null);
  const set = (d: Partial<Draft>) => setDraft((x) => ({ ...x, ...d }));
  const create = useApiMutation({
    mutationFn: async () =>
      data(await api.POST("/platform/v1/orgs/{org}/guardrails", { params: { path: { org } }, body: { kind: draft.kind, ...body(draft) } })),
    invalidates: [guardrailsKey(org)],
    onSuccess: (g) => {
      if (g.secret) return setCreated(g);
      toast.success(t("org.guardrails.created", "Guardrail added"));
      onClose();
    },
  });
  const rules = draft.kind === "rules";
  const reveal = useSecretReveal(created, false, onClose);
  const where = !draft.name.trim()
    ? t("org.guardrails.nameIt", "Give the guardrail a name")
    : !draft.url.trim()
      ? t("org.guardrails.enterUrl", "Enter its address")
      : undefined;
  const how = draft.phases.length === 0 ? t("org.guardrails.choosePhase", "Choose what it checks") : timeoutProblem(draft.timeout);
  const ruled = !draft.name.trim() ? t("org.guardrails.nameIt", "Give the guardrail a name") : rulesProblem(draft.rules);
  return (
    <>
      <StepDialog
        open
        size="md"
        onOpenChange={(o) => !o && (created ? reveal.done() : onClose())}
        result={reveal.result}
        title={t("org.guardrails.add", "Add guardrail")}
        submitLabel={t("org.guardrails.addButton", "Add guardrail")}
        submitting={create.isPending}
        onSubmit={() => (rules ? !ruled : !where && !how) && create.mutate()}
        steps={[
          {
            id: "kind",
            title: t("org.guardrails.kind", "How does it check?"),
            description: t("org.guardrails.kindHint", "Your own service gets every request; built-in rules run in Kaeso."),
            content: (
              <RadioGroup
                variant="cards"
                aria-label={t("org.guardrails.kind", "How does it check?")}
                value={draft.kind}
                onValueChange={(v) => set({ kind: v as GuardrailKind })}
              >
                <Radio
                  value="http"
                  label={t("org.guardrails.kind.http", "Your service")}
                  description={t("org.guardrails.kind.httpHint", "An HTTP service of yours allows, blocks or rewrites each request.")}
                />
                <Radio
                  value="rules"
                  label={t("org.guardrails.kind.rules", "Built-in rules")}
                  description={t("org.guardrails.kind.rulesHint", "Kaeso finds secrets and personal data and masks, blocks or counts them.")}
                />
              </RadioGroup>
            ),
          },
          ...(rules
            ? [
                {
                  id: "rules",
                  title: t("org.guardrails.rulesTitle", "What do the rules find?"),
                  description: t("org.guardrails.rulesHint", "They read the text of chat requests, tool calls and tool results."),
                  blockedReason: ruled,
                  content: (
                    <div className="flex flex-col gap-16">
                      <NameField draft={draft} set={set} error={create.error} />
                      <RulesFields org={org} rules={draft.rules} set={(r) => set({ rules: r })} />
                    </div>
                  ),
                },
              ]
            : [
                {
                  id: "where",
                  title: t("org.guardrails.where", "Where does Kaeso ask?"),
                  description: t("org.guardrails.whereHint", "Kaeso posts each request there and signs it like a webhook."),
                  blockedReason: where,
                  content: <WhereFields draft={draft} set={set} error={create.error} />,
                },
                {
                  id: "how",
                  title: t("org.guardrails.how", "What does it check?"),
                  description: t("org.guardrails.howHint", "Chat requests only; embeddings, media and token counts pass unchecked."),
                  blockedReason: how,
                  content: <HowFields draft={draft} set={set} error={create.error} />,
                },
              ]),
        ]}
      />
      {reveal.confirm}
    </>
  );
}

function EditGuardrailDialog({ org, guardrail, onClose }: { org: string; guardrail: GuardrailView; onClose: () => void }) {
  const [draft, setDraft] = useState(draftOf(guardrail));
  const set = (d: Partial<Draft>) => setDraft((x) => ({ ...x, ...d }));
  const save = useApiMutation({
    mutationFn: async () =>
      data(
        await api.PATCH("/platform/v1/orgs/{org}/guardrails/{guardrail}", {
          params: { path: { org, guardrail: guardrail.id }, header: { "If-Match": `"${guardrail.revision}"` } },
          body: body(draft),
        }),
      ),
    invalidates: [guardrailsKey(org), ["orgs", org, "projects"]],
    onSuccess: (g) => {
      toast.success(t("org.guardrails.saved", "{name} saved", { name: g.name }));
      onClose();
    },
  });
  const rules = guardrail.kind === "rules";
  const blocked = rules
    ? !draft.name.trim()
      ? t("org.guardrails.nameIt", "Give the guardrail a name")
      : rulesProblem(draft.rules)
    : !draft.name.trim() || !draft.url.trim()
      ? t("org.guardrails.nameAndUrl", "Enter a name and an address")
      : draft.phases.length === 0
        ? t("org.guardrails.choosePhase", "Choose what it checks")
        : timeoutProblem(draft.timeout);
  return (
    <Dialog
      open
      busy={save.isPending}
      onOpenChange={(o) => !o && onClose()}
      title={t("org.guardrails.editTitle", "Edit {name}", { name: guardrail.name })}
      description={t("org.guardrails.editHint", "Every project that uses it gets the change at once.")}
      onSubmit={() => !blocked && save.mutate()}
      footer={
        <>
          <Dialog.Close>
            <Button>{common.cancel()}</Button>
          </Dialog.Close>
          <Button
            variant="primary"
            loading={save.isPending}
            disabled={!!blocked}
            {...(blocked ? { disabledReason: blocked } : {})}
            onClick={() => save.mutate()}
          >
            {common.save()}
          </Button>
        </>
      }
    >
      {rules ? (
        <div className="flex flex-col gap-16">
          <NameField draft={draft} set={set} error={save.error} />
          <RulesFields org={org} rules={draft.rules} set={(r) => set({ rules: r })} />
        </div>
      ) : (
        <div className="flex flex-col gap-24">
          <WhereFields draft={draft} set={set} error={save.error} />
          <HowFields draft={draft} set={set} error={save.error} />
        </div>
      )}
    </Dialog>
  );
}

function useSecretReveal(created: GuardrailWithSecret | null, rotated: boolean, onDone: () => void) {
  const [copied, setCopied] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const done = () => (copied ? onDone() : setConfirm(true));
  const result: StepDialogResult | undefined = created
    ? {
        size: "lg",
        title: rotated ? t("org.guardrails.rotated", "New signing secret") : t("org.guardrails.created", "Guardrail added"),
        description: t("org.guardrails.secretHint", "Copy the signing secret for {name} now; your service checks requests with it.", {
          name: created.guardrail.name,
        }),
        footer: <Button onClick={done}>{common.finish()}</Button>,
        content: (
          <SecretReveal
            secret={String(created.secret)}
            name={created.guardrail.name}
            baseUrl={window.location.origin}
            snippets={false}
            onCopied={() => setCopied(true)}
          />
        ),
      }
    : undefined;
  return {
    result,
    done,
    confirm: (
      <AlertDialog
        open={confirm}
        onOpenChange={setConfirm}
        tone="warning"
        title={t("org.guardrails.copiedConfirm", "Did you copy the secret?")}
        description={t("org.guardrails.copiedConfirmHint", "It can't be shown again; without it, give the guardrail a new one.")}
        confirmLabel={t("org.platformKeys.closeWithoutCopying", "Close without copying")}
        cancelLabel={t("org.platformKeys.goBack", "Go back")}
        onConfirm={onDone}
      />
    ),
  };
}

export function SecretDialog({ created, onClose }: { created: GuardrailWithSecret; onClose: () => void }) {
  const reveal = useSecretReveal(created, true, onClose);
  return (
    <>
      <Dialog
        open
        size="lg"
        onOpenChange={(o) => !o && reveal.done()}
        title={t("org.guardrails.rotated", "New signing secret")}
        description={reveal.result?.description}
        footer={reveal.result?.footer}
      >
        {reveal.result?.content}
      </Dialog>
      {reveal.confirm}
    </>
  );
}
