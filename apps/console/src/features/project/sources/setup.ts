import type { CodeTab } from "@logicplanes/kaeso-ui";
import type { SourceKindView } from "~/api/generated/types";
import { t } from "~/i18n";

/** What a setup snippet is written for: the source's kind, where it sends, and the key it sends with. */
export interface SetupTarget {
  kind: SourceKindView;
  /** The install's ingest address, such as `https://kaeso.example.com/ingest`. */
  ingestUrl: string;
  /** The source's key, or a placeholder where it was shown once and is gone. */
  secret: string;
  /** Whether the source keeps the prompts, replies and tool arguments its client sends. */
  content: boolean;
}

// Where each snippet goes: file names, the same in every language.
const CLAUDE_SETTINGS = "~/.claude/settings.json";
const CODEX_CONFIG = "~/.codex/config.toml";
const GEMINI_SETTINGS = "~/.gemini/settings.json";
const ANTIGRAVITY_HOOKS = "~/.gemini/config/hooks.json";
const LITELLM_CONFIG = "config.yaml";
const shellLabel = () => t("project.sources.setup.shell", "Shell");

/** The placeholder a snippet carries once the key is no longer shown. */
export const KEY_PLACEHOLDER = "<your source key>";

const shell = (lines: [string, string][]) => lines.map(([k, v]) => `export ${k}=${v}`).join("\n");
const json = (value: unknown) => JSON.stringify(value, null, 2);

function claudeCode({ ingestUrl, secret, content }: SetupTarget): CodeTab[] {
  const env: [string, string][] = [
    ["CLAUDE_CODE_ENABLE_TELEMETRY", "1"],
    ["OTEL_LOGS_EXPORTER", "otlp"],
    ["OTEL_METRICS_EXPORTER", "otlp"],
    ["OTEL_EXPORTER_OTLP_PROTOCOL", "http/protobuf"],
    ["OTEL_EXPORTER_OTLP_ENDPOINT", `${ingestUrl}/otlp`],
    ["OTEL_EXPORTER_OTLP_HEADERS", `"Authorization=Bearer ${secret}"`],
    ["OTEL_LOG_TOOL_DETAILS", "1"],
    ...(content ? ([["OTEL_LOG_USER_PROMPTS", "1"]] as [string, string][]) : []),
  ];
  return [
    { label: CLAUDE_SETTINGS, lang: "json", code: json({ env: Object.fromEntries(env.map(([k, v]) => [k, v.replace(/^"|"$/g, "")])) }) },
    { label: shellLabel(), lang: "bash", code: shell(env) },
  ];
}

function codex({ ingestUrl, secret, content }: SetupTarget): CodeTab[] {
  return [
    {
      label: CODEX_CONFIG,
      lang: "toml",
      code: [
        "[otel]",
        `log_user_prompt = ${content}`,
        `exporter = { otlp-http = { endpoint = "${ingestUrl}/otlp/v1/logs", protocol = "binary", headers = { authorization = "Bearer ${secret}" } } }`,
      ].join("\n"),
    },
  ];
}

function geminiCli({ ingestUrl, secret, content }: SetupTarget): CodeTab[] {
  const telemetry = {
    enabled: true,
    target: "local",
    otlpProtocol: "http",
    otlpEndpoint: `${ingestUrl}/otlp`,
    otlpHeaders: { Authorization: `Bearer ${secret}` },
    logPrompts: content,
  };
  return [
    { label: GEMINI_SETTINGS, lang: "json", code: json({ telemetry }) },
    {
      label: shellLabel(),
      lang: "bash",
      code: shell([
        ["GEMINI_TELEMETRY_ENABLED", "true"],
        ["GEMINI_TELEMETRY_TARGET", "local"],
        ["GEMINI_TELEMETRY_OTLP_PROTOCOL", "http"],
        ["GEMINI_TELEMETRY_OTLP_ENDPOINT", `${ingestUrl}/otlp`],
        ["GEMINI_TELEMETRY_OTLP_HEADERS", `"Authorization=Bearer ${secret}"`],
        ["GEMINI_TELEMETRY_LOG_PROMPTS", String(content)],
      ]),
    },
  ];
}

const HOOK_SCRIPT = "~/.gemini/config/kaeso-source.sh";

function antigravity({ ingestUrl, secret, content }: SetupTarget): CodeTab[] {
  const fields = [
    "type",
    "step_index",
    "created_at",
    "model",
    "input_tokens",
    "cache_read_tokens",
    "cache_write_tokens",
    "cache_creation_input_tokens",
    "output_tokens",
    ...(content ? ["content"] : []),
  ];
  const script = [
    "#!/bin/sh",
    "# Reports Antigravity's model calls to a Kaeso source after each invocation. Needs curl and jq.",
    `KAESO_INGEST="${ingestUrl}/v1/antigravity"`,
    `KAESO_SOURCE_KEY="${secret}"`,
    "hook=$(cat)",
    "printf '{}'",
    "log=$(printf '%s' \"$hook\" | jq -r '.transcriptPath // empty')",
    '[ -n "$log" ] || exit 0',
    'full="$(dirname "$log")/transcript_full.jsonl"',
    '[ -f "$full" ] || exit 0',
    `jq -cs --argjson hook "$hook" '{hook: $hook, steps: [.[] | select(.type == "PLANNER_RESPONSE") | {${fields.join(", ")}}]}' "$full" |`,
    '  curl -fsS -m 10 "$KAESO_INGEST" -H "Authorization: Bearer $KAESO_SOURCE_KEY" \\',
    '    -H "Content-Type: application/json" --data-binary @- > /dev/null 2>&1',
    "exit 0",
  ].join("\n");
  return [
    { label: HOOK_SCRIPT, lang: "bash", code: script },
    {
      label: ANTIGRAVITY_HOOKS,
      lang: "json",
      code: json({ kaeso: { PostInvocation: [{ type: "command", command: `sh ${HOOK_SCRIPT}`, timeout: 15 }] } }),
    },
  ];
}

function opentelemetry({ ingestUrl, secret }: SetupTarget): CodeTab[] {
  return [
    {
      label: t("project.sources.setup.environment", "Environment"),
      lang: "bash",
      code: shell([
        ["OTEL_EXPORTER_OTLP_ENDPOINT", `${ingestUrl}/otlp`],
        ["OTEL_EXPORTER_OTLP_PROTOCOL", "http/protobuf"],
        ["OTEL_EXPORTER_OTLP_HEADERS", `"Authorization=Bearer%20${secret}"`],
        ["OTEL_SEMCONV_STABILITY_OPT_IN", "gen_ai_latest_experimental"],
      ]),
    },
    {
      label: t("project.sources.setup.collector", "Collector"),
      lang: "yaml",
      code: [
        "exporters:",
        "  otlphttp/kaeso:",
        `    endpoint: ${ingestUrl}/otlp`,
        "    headers:",
        `      Authorization: Bearer ${secret}`,
        "service:",
        "  pipelines:",
        "    traces:",
        "      exporters: [otlphttp/kaeso]",
        "    logs:",
        "      exporters: [otlphttp/kaeso]",
      ].join("\n"),
    },
  ];
}

function http({ ingestUrl, secret }: SetupTarget): CodeTab[] {
  const body = {
    requests: [
      {
        id: "req_0001",
        started_at: "2026-11-23T09:30:00Z",
        duration_ms: 1200,
        provider: "openai",
        model: "gpt-5.4-mini",
        status: 200,
        input_tokens: 1200,
        output_tokens: 300,
        email: "ada@example.com",
      },
    ],
  };
  return [
    {
      label: "curl",
      lang: "bash",
      code: [
        `curl ${ingestUrl}/v1/events \\`,
        `  -H "Authorization: Bearer ${secret}" \\`,
        '  -H "Content-Type: application/json" \\',
        `  -d '${JSON.stringify(body)}'`,
      ].join("\n"),
    },
  ];
}

function litellm({ ingestUrl, secret }: SetupTarget): CodeTab[] {
  return [
    {
      label: LITELLM_CONFIG,
      lang: "yaml",
      code: [
        "litellm_settings:",
        '  callbacks: ["kaeso"]',
        "callback_settings:",
        "  kaeso:",
        "    callback_type: generic_api",
        `    endpoint: ${ingestUrl}/v1/litellm`,
        "    headers:",
        `      Authorization: Bearer ${secret}`,
      ].join("\n"),
    },
  ];
}

function openrouter({ ingestUrl, secret }: SetupTarget): CodeTab[] {
  return [
    { label: t("project.sources.setup.endpoint", "Endpoint"), lang: "text", code: `${ingestUrl}/otlp/v1/traces` },
    { label: t("project.sources.setup.headers", "Headers"), lang: "json", code: json({ Authorization: `Bearer ${secret}` }) },
  ];
}

const SNIPPETS: Record<SourceKindView, (t: SetupTarget) => CodeTab[]> = {
  claude_code: claudeCode,
  codex,
  gemini_cli: geminiCli,
  antigravity,
  opentelemetry,
  http,
  litellm,
  openrouter,
};

/** The exact lines that make a source's client report to Kaeso, one tab per place they can go. */
export function setupSnippets(target: SetupTarget): CodeTab[] {
  return SNIPPETS[target.kind](target);
}
