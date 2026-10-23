import { useMemo, useState, type ComponentProps, type ReactNode } from "react";
import { Button, Code, Input, Link, ProviderPicker, Radio, RadioCards, SensitiveInput, StepDialog, icons } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { AccountAttempt, AccountSignInView, CredentialView, ProviderView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";
import { toast } from "~/shell/toast";
import { keyTitle, providerOptions, useProviders } from "./providers";

type DialogStep = ComponentProps<typeof StepDialog>["steps"][number];
type Method = "api_key" | "account";

function keyHelp(provider: ProviderView | undefined): ReactNode {
  if (!provider) return t("kaeso.providerKey.help", "Paste the provider's API key.");
  const page = provider.key_page;
  const site = (
    <Link href={page.url} external>
      {new URL(page.url).host}
    </Link>
  );
  return provider.key_prefix
    ? tRich("kaeso.providerKey.where", "Create one at {site} under {page}; it starts with {prefix}.", {
        site,
        page: page.label,
        prefix: <Code className="text-nowrap">{provider.key_prefix}</Code>,
      })
    : tRich("kaeso.providerKey.whereSite", "Create one at {site} under {page}.", { site, page: page.label });
}

function keyPlaceholder(provider: ProviderView | undefined): string {
  return `${provider ? (provider.key_prefix ?? "") : "sk-"}…`;
}

function openBlankTab(): Window | null {
  const tab = window.open("about:blank", "_blank");
  if (tab) tab.opener = null;
  return tab;
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
  const providers = useProviders(org);
  const [provider, setProvider] = useState<string | null>(replace?.provider ?? null);
  const [name, setName] = useState("");
  const [secret, setSecret] = useState("");
  const [makeDefault, setMakeDefault] = useState(false);
  const [method, setMethod] = useState<Method>(replace && replace.auth_method !== "api_key" ? "account" : "api_key");
  const [attempt, setAttempt] = useState<AccountAttempt | null>(null);
  const [blocked, setBlocked] = useState(false);
  const [callback, setCallback] = useState("");
  const chosen = provider ?? "";
  const label = provider ? providers.name(provider) : "";
  const known = provider ? providers.get(provider) : undefined;
  const account: AccountSignInView | null = known?.account ?? null;
  const signingIn = method === "account" && account !== null;
  const current = provider ? keys.find((k) => k.provider === provider && k.is_default) : undefined;
  const choices = useMemo(() => providerOptions(providers, keys), [providers, keys]);
  const begin = useApiMutation({
    mutationFn: async (_tab: Window | null) => {
      if (replace) return data(await api.POST("/platform/v1/orgs/{org}/credentials/{credential}/oauth", { params: { path: { org, credential: replace.id } } }));
      return data(
        await api.POST("/platform/v1/orgs/{org}/credentials/oauth", {
          params: { path: { org } },
          body: { provider: chosen, name: name.trim(), make_default: makeDefault },
        }),
      );
    },
    onSuccess: (started, tab) => {
      setAttempt(started);
      setCallback("");
      setBlocked(!tab);
      if (tab) tab.location.href = started.authorize_url;
    },
    onError: (_error, tab) => tab?.close(),
  });
  const save = useApiMutation({
    mutationFn: async () => {
      if (signingIn && attempt && replace)
        return data(
          await api.POST("/platform/v1/orgs/{org}/credentials/{credential}/oauth/{attempt}", {
            params: { path: { org, credential: replace.id, attempt: attempt.attempt } },
            body: { callback_url: callback.trim() },
          }),
        );
      if (signingIn && attempt)
        return data(
          await api.POST("/platform/v1/orgs/{org}/credentials/oauth/{attempt}", {
            params: { path: { org, attempt: attempt.attempt } },
            body: { callback_url: callback.trim() },
          }),
        );
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
          body: { provider: chosen, name: name.trim(), secret: secret.trim(), make_default: makeDefault },
        }),
      );
    },
    invalidates: [["orgs", org, "credentials"]],
    onSuccess: (k) => {
      setSecret("");
      setCallback("");
      toast.success(
        replace && signingIn
          ? t("kaeso.providerKey.signedInAgain", "{title} signed in again", { title: keyTitle(k, providers) })
          : replace
            ? t("kaeso.providerKey.replaced", "{title} replaced", { title: keyTitle(k, providers) })
            : k.is_default
              ? t("kaeso.providerKey.isDefault", "{name} is the {provider} default", { name: k.name, provider: label })
              : t("kaeso.providerKey.added", "{title} added", { title: keyTitle(k, providers) }),
      );
      onClose();
    },
  });

  const keyStep: DialogStep = {
    id: "key",
    title: replace
      ? t("kaeso.providerKey.pasteNew", "Paste the new {provider} key", { provider: label })
      : t("kaeso.providerKey.paste", "Paste your {provider} API key", { provider: label }),
    description: provider ? keyHelp(known) : undefined,
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
          placeholder={provider ? keyPlaceholder(known) : undefined}
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
  const product = account?.product ?? "";
  const accountStep: DialogStep = {
    id: "account",
    title: t("kaeso.providerKey.signInTitle", "Sign in with your {product} account", { product }),
    description: t("kaeso.providerKey.ownAccount", "Use your own account only; sharing it breaks {provider}'s terms.", { provider: label }),
    blockedReason:
      !replace && !name.trim()
        ? t("kaeso.providerKey.nameBlocked", "Give the key a name")
        : !attempt
          ? t("kaeso.providerKey.continueFirst", "Continue with {product} first", { product })
          : !callback.trim()
            ? t("kaeso.providerKey.pasteAddressBlocked", "Paste the address of the page that didn't load")
            : undefined,
    content: (
      <div className="flex flex-col items-start gap-12">
        {!replace && (
          <Input
            className="w-full"
            label={t("common.name", "Name")}
            placeholder={t("kaeso.providerKey.accountNamePlaceholder", "My {product} plan", { product })}
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setAttempt(null);
            }}
            maxLength={100}
            autoFocus
          />
        )}
        <Button
          leadingIcon={icons.openExternal}
          loading={begin.isPending}
          disabled={!replace && !name.trim()}
          {...(!replace && !name.trim() ? { disabledReason: t("kaeso.providerKey.nameBlocked", "Give the key a name") } : {})}
          onClick={() => begin.mutate(openBlankTab())}
        >
          {t("kaeso.providerKey.continueWith", "Continue with {product}", { product })}
        </Button>
        {attempt && (
          <>
            {blocked && (
              <Link href={attempt.authorize_url} external>
                {t("kaeso.providerKey.openSignIn", "Open the {product} sign-in page", { product })}
              </Link>
            )}
            <Input
              className="w-full"
              label={t("kaeso.providerKey.callback", "Address of the page")}
              description={t("kaeso.providerKey.callbackHint", "After you approve, the page can't load. That's expected. Paste its address here.")}
              value={callback}
              onChange={(e) => setCallback(e.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
          </>
        )}
      </div>
    ),
  };
  const methodStep: DialogStep = {
    id: "method",
    title: t("kaeso.providerKey.methodTitle", "Add an API key or sign in with {product}?", { product }),
    content: (
      <RadioCards
        aria-label={t("kaeso.providerKey.method", "Sign-in method")}
        value={method}
        onValueChange={(v) => setMethod(v === "account" ? "account" : "api_key")}
      >
        <Radio
          value="api_key"
          label={t("kaeso.providerKey.apiKey", "API key")}
          description={t("kaeso.providerKey.apiKeyHint", "Requests are billed to the {provider} account of the key.", { provider: label })}
        />
        <Radio
          value="account"
          label={t("kaeso.providerKey.account", "{product} account", { product })}
          description={t("kaeso.providerKey.accountHint", "Requests use the plan of the {product} account you sign in with.", { product })}
        />
      </RadioCards>
    ),
  };
  const defaultStep: DialogStep[] = current
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
    : [];
  const choose = (p: string) => {
    setProvider(p);
    setMethod("api_key");
    setAttempt(null);
  };
  const providerStep: DialogStep = {
    id: "provider",
    title: t("kaeso.providerKey.which", "Which provider is the key for?"),
    blockedReason: provider ? undefined : t("kaeso.providerKey.chooseProvider", "Choose a provider"),
    content: (
      <ProviderPicker aria-label={t("common.provider", "Provider")} providers={choices} value={provider} onValueChange={(p) => p && choose(p)} autoFocus />
    ),
  };
  const steps: DialogStep[] = replace
    ? [signingIn ? accountStep : keyStep]
    : [providerStep, ...(account ? [methodStep] : []), ...(signingIn ? [...defaultStep, accountStep] : [keyStep, ...defaultStep])];
  const ready = signingIn ? !!attempt && !!callback.trim() && (replace || name.trim()) : secret.trim() && !/\s/.test(secret.trim()) && (replace || name.trim());

  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={
        replace && signingIn
          ? t("kaeso.providerKey.signInAgainTitle", "Sign in again to {provider} · {name}", { provider: label, name: replace.name })
          : replace
            ? t("kaeso.providerKey.replaceTitle", "Replace {provider} · {name}", { provider: providers.name(replace.provider), name: replace.name })
            : t("kaeso.providerKey.addTitle", "Add provider key")
      }
      submitLabel={
        replace && signingIn
          ? t("kaeso.providerKey.signInAgain", "Sign in again")
          : replace
            ? t("kaeso.providerKey.replace", "Replace key")
            : signingIn
              ? t("kaeso.providerKey.addAccount", "Add account")
              : t("kaeso.providerKey.add", "Add key")
      }
      submitting={save.isPending}
      onSubmit={() => provider && ready && save.mutate()}
      steps={steps}
    />
  );
}
