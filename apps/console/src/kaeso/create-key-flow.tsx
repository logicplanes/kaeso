import { useRef, useState } from "react";
import { flushSync } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { ModelPicker } from "@logicplanes/kaeso-ui/playground";
import {
  AlertDialog,
  Button,
  DialogScope,
  Input,
  Radio,
  RadioCards,
  SecretReveal,
  Select,
  StepDialog,
  icons,
  type StepDialogResult,
} from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { CreatedKey_KeyView } from "~/api/generated/types";
import { catalogueQuery } from "~/api/queries/models";
import { modelAccessQuery, serviceAccountsQuery, type Principal } from "~/api/queries/members";
import { orgQuery } from "~/api/queries/orgs";
import { projectQuery } from "~/api/queries/projects";
import { useApiMutation } from "~/auth/mutation";
import { useSession } from "~/auth/session";
import { catalogueModelOption, keepChosen } from "./model-options";
import { t } from "~/i18n";
import { ExpiryPicker, defaultExpiry, expiryRequest, type Expiry } from "./expiry-picker";
import { LimitEditor, limitBody, limitProblem, newLimit, type LimitDraft } from "./limit-editor";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";

type Access = "inference" | "platform" | "both";

const EXAMPLE_NAME = "checkout-web";

interface CreateKeyProps {
  org: string;
  project: string;
  open: boolean;
  onClose: () => void;
  fixedServiceAccount?: { id: string; name: string } | undefined;
}

export function CreateKeyDialog(props: CreateKeyProps) {
  return (
    <DialogScope open={props.open}>
      <CreateKeyFlow {...props} />
    </DialogScope>
  );
}

function CreateKeyFlow({ org, project, open, onClose, fixedServiceAccount }: CreateKeyProps) {
  const session = useSession();
  const rights = useProjectRights(org, project);
  const needs = useNeedsRight(org, project);
  const canForAccounts = rights.has("service_accounts.keys.create");
  const canPlatform = rights.has("keys.platform_access.grant");
  const accounts = useQuery({ ...serviceAccountsQuery(org, project), enabled: canForAccounts && !fixedServiceAccount });
  const projectView = useQuery(projectQuery(org, project));
  const orgView = useQuery(orgQuery(org));
  const catalogue = useQuery(catalogueQuery(org, project));

  const [name, setName] = useState("");
  const [owner, setOwner] = useState<string>(fixedServiceAccount ? fixedServiceAccount.id : "me");
  const [picked, setExpiry] = useState<Expiry | null>(null);
  const [date, setDate] = useState<Date | null>(null);
  const [access, setAccess] = useState<Access>("inference");
  const [models, setModels] = useState<string[]>([]);
  const [limits, setLimits] = useState<LimitDraft[]>([]);
  const lastLimit = useRef<HTMLDivElement>(null);
  const [created, setCreated] = useState<{ key: CreatedKey_KeyView; models: string[] } | null>(null);
  const reveal = useKeyReveal(org, project, created, onClose);

  const principal: Principal = owner === "me" ? { kind: "user", id: session.user.id } : { kind: "service_account", id: owner };
  const effective = useQuery({ ...modelAccessQuery(org, project, principal), enabled: rights.has("limits.view") });
  const allowedModels = (catalogue.data ?? []).filter((m) => !effective.data || effective.data.effective.includes(m.id));
  const allowed = new Set(allowedModels.map((m) => m.id));
  const keptModels = models.filter((id) => allowed.has(id));
  const keptLimits = limits.map((l) => (l.model && !allowed.has(l.model) ? { ...l, model: null } : l));
  const maxDays = projectView.data?.max_key_lifetime_days ?? orgView.data?.max_key_lifetime_days ?? null;
  const expiry: Expiry = picked ?? defaultExpiry(maxDays);

  const problems = keptLimits.map(limitProblem).filter(Boolean);
  const ready = name.trim().length > 0 && (expiry !== "date" || date !== null) && problems.length === 0;

  const create = useApiMutation({
    mutationFn: async () => {
      const body = {
        name: name.trim(),
        access,
        ...expiryRequest(expiry, date),
        ...(keptModels.length ? { models: keptModels } : {}),
        ...(keptLimits.length ? { limits: keptLimits.map(limitBody) } : {}),
      };
      if (owner === "me") return data(await api.POST("/platform/v1/orgs/{org}/projects/{project}/keys", { params: { path: { org, project } }, body }));
      return data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/service-accounts/{account}/keys", {
          params: { path: { org, project, account: owner } },
          body,
        }),
      );
    },
    invalidates: [
      ["orgs", org, "projects", project, "keys"],
      ["me", "keys"],
    ],
    onSuccess: (k) => setCreated({ key: k, models: keptModels }),
  });

  const ownerOptions = [
    { value: "me", label: t("kaeso.createKey.forMe", "Me ({name})", { name: session.user.name }) },
    ...(accounts.data ?? [])
      .filter((a) => !a.archived_at)
      .map((a) => ({ value: a.id, label: t("kaeso.createKey.forServiceAccount", "Service account {name}", { name: a.name }) })),
  ];

  const forStep =
    !fixedServiceAccount && canForAccounts && (accounts.isPending || ownerOptions.length > 1) ? (
      <Select
        label={t("kaeso.createKey.for", "For")}
        description={t("kaeso.createKey.forHint", "A service account's key acts with the service account's rights.")}
        options={ownerOptions}
        value={owner}
        onValueChange={(v) => setOwner(v as string)}
        disabled={accounts.isPending}
        fullWidth
      />
    ) : null;

  return (
    <>
      <StepDialog
        open={open}
        onOpenChange={(o) => !o && (created ? reveal.done() : onClose())}
        result={reveal.result}
        title={t("kaeso.createKey.title", "Create key")}
        submitLabel={t("kaeso.createKey.title", "Create key")}
        submitting={create.isPending}
        onSubmit={() => ready && create.mutate()}
        steps={[
          {
            id: "name",
            title: fixedServiceAccount
              ? t("kaeso.createKey.keyFor", "A key for {name}", { name: fixedServiceAccount.name })
              : t("kaeso.createKey.whatFor", "What is it for?"),
            description: t("kaeso.createKey.nameHint", "Applications send the key with every request; its name shows in request logs and alerts."),
            blockedReason: name.trim() ? undefined : t("kaeso.createKey.enterName", "Enter a name"),
            content: (
              <div className="flex flex-col gap-16">
                <Input
                  label={t("common.name", "Name")}
                  placeholder={EXAMPLE_NAME}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  autoFocus
                  maxLength={100}
                />
                {forStep}
              </div>
            ),
          },
          {
            id: "expiry",
            title: t("kaeso.createKey.whenExpires", "When does it expire?"),
            blockedReason: expiry === "date" && !date ? t("kaeso.createKey.pickDay", "Pick the day it expires") : undefined,
            content: <ExpiryPicker value={expiry} onChange={setExpiry} date={date} onDateChange={setDate} maxDays={maxDays} />,
          },
          {
            id: "access",
            title: t("kaeso.createKey.whatCalls", "What can it call?"),
            content: (
              <RadioCards aria-label={t("kaeso.createKey.access", "Access")} value={access} onValueChange={(v) => setAccess(v as Access)}>
                <Radio
                  value="inference"
                  label={t("format.access.inference", "Inference")}
                  description={t("kaeso.createKey.inferenceHint", "Calls the model API.")}
                />
                <Radio
                  value="platform"
                  label={t("format.access.platform", "Platform")}
                  description={
                    canPlatform
                      ? t("kaeso.createKey.platformHint", "Calls the Platform API with its owner's rights, in this project. Needs a recent sign-in.")
                      : needs("keys.platform_access.grant")
                  }
                  disabled={!canPlatform}
                />
                <Radio
                  value="both"
                  label={t("kaeso.createKey.both", "Inference and platform")}
                  description={canPlatform ? t("kaeso.createKey.bothHint", "Both. Needs a recent sign-in.") : needs("keys.platform_access.grant")}
                  disabled={!canPlatform}
                />
              </RadioCards>
            ),
          },
          {
            id: "narrow",
            title: t("kaeso.createKey.narrow", "Models and limits"),
            description: t("kaeso.createKey.narrowHint", "Optional: without them it may call what its owner may, within the owner's and the project's limits."),
            blockedReason: problems[0] ?? undefined,
            content: (
              <div className="flex flex-col gap-16">
                <ModelPicker
                  label={t("common.models", "Models")}
                  multiple
                  appearance="default"
                  size="md"
                  fullWidth
                  options={keepChosen(allowedModels.map(catalogueModelOption), keptModels)}
                  value={keptModels}
                  onValueChange={setModels}
                  placeholder={t("kaeso.limits.allModels", "All models")}
                />
                {keptLimits.map((l, i) => (
                  <div key={i} ref={i === keptLimits.length - 1 ? lastLimit : undefined} className="flex flex-col gap-12">
                    <LimitEditor value={l} onChange={(d) => setLimits(keptLimits.map((x, j) => (j === i ? d : x)))} models={allowedModels} />
                    <Button
                      variant="ghost"
                      size="sm"
                      leadingIcon={icons.delete}
                      onClick={() => setLimits(keptLimits.filter((_, j) => j !== i))}
                      className="self-start"
                    >
                      {t("kaeso.limits.removeThis", "Remove this limit")}
                    </Button>
                  </div>
                ))}
                <Button
                  size="sm"
                  leadingIcon={icons.add}
                  onClick={() => {
                    flushSync(() => setLimits([...keptLimits, newLimit()]));
                    lastLimit.current?.querySelector<HTMLElement>("button")?.focus();
                  }}
                  className="self-start"
                >
                  {t("kaeso.limits.add", "Add limit")}
                </Button>
              </div>
            ),
          },
        ]}
      />
      {reveal.confirm}
    </>
  );
}

function useKeyReveal(org: string, project: string, created: { key: CreatedKey_KeyView; models: string[] } | null, onDone: () => void) {
  const [copied, setCopied] = useState(false);
  const catalogue = useQuery(catalogueQuery(org, project));
  const models = created?.models ?? [];
  const model = catalogue.data?.find((m) => !models.length || models.includes(m.id))?.model_id;
  const [confirm, setConfirm] = useState(false);
  const done = () => (copied ? onDone() : setConfirm(true));
  const result: StepDialogResult | undefined = created
    ? {
        size: "lg",
        title: t("kaeso.createKey.created", "Key created"),
        description: t("kaeso.createKey.createdHint", "Copy the key now: it's shown once, and Kaeso keeps only a hash of it."),
        footer: <Button onClick={done}>{t("common.done", "Done")}</Button>,
        content: (
          <SecretReveal
            secret={created.key.secret}
            name={created.key.name}
            baseUrl={window.location.origin}
            {...(model ? { model } : {})}
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
        title={t("kaeso.createKey.copied", "Did you copy the key?")}
        description={t("kaeso.createKey.copiedHint", "It can't be shown again; without it, you'll need a new key.")}
        confirmLabel={t("kaeso.createKey.closeWithoutCopying", "Close without copying")}
        cancelLabel={t("kaeso.createKey.goBack", "Go back")}
        onConfirm={onDone}
      />
    ),
  };
}
