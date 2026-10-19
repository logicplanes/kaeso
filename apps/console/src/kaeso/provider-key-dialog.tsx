import { useMemo, useState, type ComponentProps, type ReactNode } from "react";
import { Code, Input, Link, ProviderPicker, Radio, RadioCards, SensitiveInput, StepDialog } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { CredentialView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { providerLabel } from "~/format/labels";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";
import { toast } from "~/shell/toast";
import { providerOptions } from "./providers";

type DialogStep = ComponentProps<typeof StepDialog>["steps"][number];

const KEY_SITES: Record<string, { url: string; page: string; prefix: string }> = {
  openai: { url: "https://platform.openai.com/api-keys", page: "API keys", prefix: "sk-" },
  anthropic: { url: "https://platform.claude.com/settings/keys", page: "Settings → API keys", prefix: "sk-ant-" },
  deepseek: { url: "https://platform.deepseek.com/api_keys", page: "API keys", prefix: "sk-" },
};

function keyHelp(provider: string): ReactNode {
  const where = KEY_SITES[provider];
  if (!where) return t("kaeso.providerKey.help", "Paste the provider's API key.");
  const site = (
    <Link href={where.url} external>
      {new URL(where.url).host}
    </Link>
  );
  return tRich("kaeso.providerKey.where", "Create one at {site} under {page}; it starts with {prefix}.", {
    site,
    page: where.page,
    prefix: <Code className="text-nowrap">{where.prefix}</Code>,
  });
}

export function ProviderKeyDialog({
  org,
  keys,
  replace,
  onClose,
}: {
  org: string;
  keys: CredentialView[];
  replace?: CredentialView | undefined;
  onClose: () => void;
}) {
  const [provider, setProvider] = useState(replace?.provider ?? "openai");
  const [name, setName] = useState("");
  const [secret, setSecret] = useState("");
  const [makeDefault, setMakeDefault] = useState(false);
  const label = providerLabel(provider);
  const current = keys.find((k) => k.provider === provider && k.is_default);
  const choices = useMemo(() => providerOptions(keys), [keys]);
  const save = useApiMutation({
    mutationFn: async () => {
      if (replace)
        return data(
          await api.POST("/platform/v1/orgs/{org}/credentials/{credential}/replace", {
            params: { path: { org, credential: replace.id } },
            body: { secret: secret.trim() },
          }),
        );
      return data(
        await api.POST("/platform/v1/orgs/{org}/credentials", {
          params: { path: { org } },
          body: { provider, name: name.trim(), secret: secret.trim(), make_default: makeDefault },
        }),
      );
    },
    invalidates: [["orgs", org, "credentials"]],
    onSuccess: (k) => {
      setSecret("");
      toast.success(
        replace
          ? t("kaeso.providerKey.replaced", "{name} replaced", { name: k.name })
          : k.is_default
            ? t("kaeso.providerKey.isDefault", "{name} is the {provider} default", { name: k.name, provider: label })
            : t("kaeso.providerKey.added", "{name} added", { name: k.name }),
      );
      onClose();
    },
  });

  const keyStep: DialogStep = {
    id: "key",
    title: replace
      ? t("kaeso.providerKey.pasteNew", "Paste the new {provider} key", { provider: label })
      : t("kaeso.providerKey.paste", "Paste your {provider} API key", { provider: label }),
    description: keyHelp(provider),
    blockedReason: !secret.trim()
      ? t("kaeso.providerKey.pasteBlocked", "Paste the {provider} API key", { provider: label })
      : /\s/.test(secret.trim())
        ? t("kaeso.providerKey.noSpaces", "The key can't contain spaces or line breaks")
        : !replace && !name.trim()
          ? t("kaeso.providerKey.nameBlocked", "Give the key a name")
          : undefined,
    content: (
      <div className="flex flex-col gap-12">
        <SensitiveInput
          label={t("kaeso.providerKey.secret", "{provider} API key", { provider: label })}
          placeholder={`${KEY_SITES[provider]?.prefix ?? "sk-"}…`}
          value={secret}
          onChange={(e) => setSecret(e.target.value)}
          autoComplete="off"
          autoFocus
        />
        {!replace && (
          <Input
            label={t("common.name", "Name")}
            placeholder={t("kaeso.providerKey.namePlaceholder", "Production")}
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={100}
          />
        )}
      </div>
    ),
  };
  const steps: DialogStep[] = replace
    ? [keyStep]
    : [
        {
          id: "provider",
          title: t("kaeso.providerKey.which", "Which provider is the key for?"),
          content: (
            <ProviderPicker
              aria-label={t("common.provider", "Provider")}
              providers={choices}
              value={provider}
              onValueChange={(p) => p && setProvider(p)}
              autoFocus
            />
          ),
        },
        keyStep,
        ...(current
          ? [
              {
                id: "default",
                title: t("kaeso.providerKey.makeDefaultTitle", "Make it the {provider} default?", { provider: label }),
                description: t("kaeso.providerKey.makeDefaultHint", "Projects without an assigned key use the default."),
                content: (
                  <RadioCards
                    aria-label={t("kaeso.providerKey.default", "Default")}
                    value={makeDefault ? "yes" : "no"}
                    onValueChange={(v) => setMakeDefault(v === "yes")}
                  >
                    <Radio
                      value="no"
                      label={t("kaeso.providerKey.keepDefault", "Keep {name} as the default", { name: current.name })}
                      description={t("kaeso.providerKey.keepDefaultHint", "Assign this key to projects later.")}
                    />
                    <Radio
                      value="yes"
                      label={t("kaeso.providerKey.makeDefault", "Make this key the default")}
                      description={t("kaeso.providerKey.makeDefaultSwitch", "Projects without an assigned key switch to it.")}
                    />
                  </RadioCards>
                ),
              },
            ]
          : []),
      ];

  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={replace ? t("kaeso.providerKey.replaceTitle", "Replace {name}", { name: replace.name }) : t("kaeso.providerKey.addTitle", "Add provider key")}
      submitLabel={replace ? t("kaeso.providerKey.replace", "Replace key") : t("kaeso.providerKey.add", "Add key")}
      submitting={save.isPending}
      onSubmit={() => secret.trim() && !/\s/.test(secret.trim()) && (replace || name.trim()) && save.mutate()}
      steps={steps}
    />
  );
}
