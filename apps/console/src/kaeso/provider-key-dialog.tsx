import { useState, type ComponentProps } from "react";
import { Input, ProviderBadge, Radio, RadioCards, SensitiveInput, StepDialog } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { CredentialView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { PROVIDER_IDS, providerLabel } from "~/format/labels";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

type DialogStep = ComponentProps<typeof StepDialog>["steps"][number];

const KEY_SITES: Record<string, { site: string; prefix?: string }> = {
  openai: { site: "platform.openai.com", prefix: "sk-" },
  anthropic: { site: "console.anthropic.com", prefix: "sk-ant-" },
  "fireworks-ai": { site: "app.fireworks.ai" },
};
function keyHelp(provider: string): string {
  const where = KEY_SITES[provider];
  if (!where) return t("kaeso.providerKey.help", "Paste the provider's API key.");
  const { site, prefix } = where;
  return prefix
    ? t("kaeso.providerKey.where", "Create one at {site} under API keys; it starts with {prefix}.", { site, prefix })
    : t("kaeso.providerKey.whereSite", "Create one at {site} under API keys.", { site });
}
const keyPlaceholder = (provider: string) => `${KEY_SITES[provider] ? (KEY_SITES[provider].prefix ?? "") : "sk-"}…`;

export function ProviderKeyDialog({
  org,
  provider: chosen,
  keys,
  replace,
  onClose,
}: {
  org: string;
  provider?: string | undefined;
  keys: CredentialView[];
  replace?: CredentialView | undefined;
  onClose: () => void;
}) {
  const [provider, setProvider] = useState(replace?.provider ?? chosen ?? "openai");
  const [name, setName] = useState("");
  const [secret, setSecret] = useState("");
  const [makeDefault, setMakeDefault] = useState(false);
  const label = providerLabel(provider);
  const current = keys.find((k) => k.provider === provider && k.is_default);
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
          placeholder={keyPlaceholder(provider)}
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
        ...(chosen
          ? []
          : [
              {
                id: "provider",
                title: t("kaeso.providerKey.which", "Which provider is the key for?"),
                content: (
                  <RadioCards aria-label={t("common.provider", "Provider")} value={provider} onValueChange={setProvider}>
                    {PROVIDER_IDS.map((p) => {
                      const count = keys.filter((k) => k.provider === p).length;
                      return (
                        <Radio
                          key={p}
                          value={p}
                          label={
                            <span className="flex items-center gap-8">
                              <ProviderBadge provider={p} size="sm" />
                              {providerLabel(p)}
                            </span>
                          }
                          description={
                            count === 0
                              ? t("kaeso.providerKey.notConnected", "Not connected yet")
                              : t("kaeso.providerKey.count", "{count, plural, one {# key} other {# keys}}", { count })
                          }
                        />
                      );
                    })}
                  </RadioCards>
                ),
              },
            ]),
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
      title={
        replace
          ? t("kaeso.providerKey.replaceTitle", "Replace {name}", { name: replace.name })
          : chosen
            ? t("kaeso.providerKey.connect", "Connect {provider}", { provider: label })
            : t("kaeso.providerKey.addTitle", "Add provider key")
      }
      submitLabel={replace ? t("kaeso.providerKey.replace", "Replace key") : t("kaeso.providerKey.add", "Add key")}
      submitting={save.isPending}
      onSubmit={() => secret.trim() && !/\s/.test(secret.trim()) && (replace || name.trim()) && save.mutate()}
      steps={steps}
    />
  );
}
