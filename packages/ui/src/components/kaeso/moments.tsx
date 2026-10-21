import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { useCopy } from "../../hooks/use-copy.js";
import { Icon } from "../icon/icon.js";
import { Button } from "../button/button.js";
import { CodeBlock, type CodeTab } from "../code-block/code-block.js";
import { ClipboardText } from "../copy-button/copy-button.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_MOMENTS_VARIANTS = {
  secret: { md: { classes: "rounded-well bg-fill-1 px-20 py-16", description: "The one-time secret on a well" } },
} as const;
export const KAESO_MOMENTS_DEFAULT_VARIANTS = { secret: "md" } as const;

export interface SnippetOptions {
  baseUrl: string;
  apiKey?: string | undefined;
  model?: string | undefined;
}

export function sdkSnippets({ baseUrl, apiKey = "$KAESO_API_KEY", model = "gpt-5" }: SnippetOptions): CodeTab[] {
  const envKey = apiKey.startsWith("$") ? `os.environ["${apiKey.slice(1)}"]` : `"${apiKey}"`;
  const nodeKey = apiKey.startsWith("$") ? `process.env.${apiKey.slice(1)}` : `"${apiKey}"`;
  return [
    {
      label: "Python",
      lang: "python",
      code: `from openai import OpenAI\n${apiKey.startsWith("$") ? "import os\n" : ""}\nclient = OpenAI(\n    base_url="${baseUrl}/v1",\n    api_key=${envKey},\n)\n\nresponse = client.responses.create(\n    model="${model}",\n    input="Write one sentence about gateways.",\n)\nprint(response.output_text)`,
    },
    {
      label: "Node",
      lang: "ts",
      code: `import OpenAI from "openai";\n\nconst client = new OpenAI({\n  baseURL: "${baseUrl}/v1",\n  apiKey: ${nodeKey},\n});\n\nconst response = await client.responses.create({\n  model: "${model}",\n  input: "Write one sentence about gateways.",\n});\nconsole.log(response.output_text);`,
    },
    {
      label: "curl",
      lang: "bash",
      code: `${apiKey.startsWith("$") ? "" : `KAESO_API_KEY=${apiKey}\n`}curl ${baseUrl}/v1/responses \\\n  -H "Authorization: Bearer ${apiKey.startsWith("$") ? apiKey : "$KAESO_API_KEY"}" \\\n  -H "Content-Type: application/json" \\\n  -d '{\n    "model": "${model}",\n    "input": "Write one sentence about gateways."\n  }'`,
    },
  ];
}

export interface SecretRevealProps extends SnippetOptions {
  secret: string;
  name?: ReactNode | undefined;
  onCopied?: (() => void) | undefined;
  snippets?: CodeTab[] | false | undefined;
  className?: string | undefined;
}

export const SecretReveal = forwardRef<HTMLDivElement, SecretRevealProps>(function SecretReveal(
  { secret, name, baseUrl, model, onCopied, snippets, className },
  ref,
) {
  const t = useUiText();
  const { copied, copy } = useCopy(2400);
  const copyKey = t("secretReveal.copyKey", "Copy key");
  const done = t("secretReveal.copied", "Copied");
  return (
    <div ref={ref} className={cn("flex flex-col gap-24", className)}>
      <div className="flex flex-col items-center gap-16 text-center">
        <div
          tabIndex={0}
          role="textbox"
          aria-readonly
          aria-label={typeof name === "string" ? t("secretReveal.secretFor", "Secret for {name}", { name }) : t("secretReveal.secret", "Secret")}
          onClick={(e) => {
            const range = document.createRange();
            range.selectNodeContents(e.currentTarget);
            const selection = window.getSelection();
            selection?.removeAllRanges();
            selection?.addRange(range);
          }}
          className="w-full cursor-text rounded-well bg-fill-1 px-20 py-16 mono text-body-lg leading-code break-all text-fg select-all focus-ring"
        >
          {secret}
        </div>
        <Button
          variant="primary"
          size="xl"
          leadingIcon={copied ? icons.check : icons.copy}
          reserveLabels={[copyKey, done]}
          onClick={async () => {
            if (await copy(secret)) onCopied?.();
          }}
        >
          {copied ? done : copyKey}
        </Button>
        <p className="inline-flex items-center gap-6 text-body text-warning-text">
          <Icon icon={icons.warning} size="sm" />
          {t("secretReveal.shownOnce", "Shown once. Store it now.")}
        </p>
      </div>
      {snippets !== false && <CodeBlock tabs={snippets ?? sdkSnippets({ baseUrl, apiKey: secret, ...(model ? { model } : {}) })} copy />}
    </div>
  );
});
SecretReveal.displayName = "SecretReveal";

export const EnvironmentSnippet = forwardRef<HTMLDivElement, SnippetOptions & { className?: string | undefined }>(function EnvironmentSnippet(
  { baseUrl, apiKey = "$KAESO_API_KEY", model, className },
  ref,
) {
  const t = useUiText();
  return (
    <div ref={ref} className={cn("flex flex-col gap-16", className)}>
      <div className="grid gap-12 md:grid-cols-2">
        <div className="flex min-w-0 flex-col gap-6">
          <span className="text-small font-medium text-fg-secondary">{t("environmentSnippet.baseUrl", "Base URL")}</span>
          <ClipboardText value={`${baseUrl}/v1`} label={t("environmentSnippet.copyBaseUrl", "Copy base URL")} />
        </div>
        <div className="flex min-w-0 flex-col gap-6">
          <span className="text-small font-medium text-fg-secondary">{t("environmentSnippet.apiKey", "API key")}</span>
          <ClipboardText value={apiKey} label={t("environmentSnippet.copyVariableName", "Copy variable name")} />
        </div>
      </div>
      <CodeBlock tabs={sdkSnippets({ baseUrl, apiKey, ...(model ? { model } : {}) })} />
    </div>
  );
});
EnvironmentSnippet.displayName = "EnvironmentSnippet";
