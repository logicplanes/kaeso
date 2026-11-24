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

// Grok Build takes its collector's headers only from the environment, never from its config file.
function grok({ ingestUrl, secret, content }: SetupTarget): CodeTab[] {
  const env: [string, string][] = [
    ["GROK_EXTERNAL_OTEL", "1"],
    ["OTEL_LOGS_EXPORTER", "otlp"],
    ["OTEL_EXPORTER_OTLP_PROTOCOL", "http/protobuf"],
    ["OTEL_EXPORTER_OTLP_ENDPOINT", `${ingestUrl}/otlp`],
    ["OTEL_EXPORTER_OTLP_HEADERS", `"Authorization=Bearer ${secret}"`],
    ["OTEL_LOG_TOOL_DETAILS", "1"],
    ...(content
      ? ([
          ["OTEL_LOG_USER_PROMPTS", "1"],
          ["OTEL_LOG_TOOL_CONTENT", "1"],
        ] as [string, string][])
      : []),
  ];
  return [{ label: shellLabel(), lang: "bash", code: shell(env) }];
}

function codex({ ingestUrl, secret, content }: SetupTarget): CodeTab[] {
  return [
    {
      label: CODEX_CONFIG,
      lang: "toml",
      code: [
        "# Merge into the [otel] table if the file has one already.",
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
    ...(content ? ["content", "thinking"] : []),
  ];
  // A model call's tool calls run as the steps right after it; their names, and with content their
  // arguments and results, let Kaeso log the MCP calls among them.
  const calls = content ? "tool_calls: (.tool_calls // [] | map({name, args}))" : "tool_calls: (.tool_calls // [] | map({name}))";
  const result = `{step_index, type, status, created_at${content ? ", content" : ""}}`;
  // Answers Antigravity at once and sends in the background, so no model call waits for Kaeso;
  // sends only the steps added since the last time, at most 1,000 at a time.
  const script = [
    "#!/bin/sh",
    "# Reports Antigravity's model calls to a Kaeso source after each invocation. Needs curl and jq.",
    `KAESO_INGEST="${ingestUrl}/v1/antigravity"`,
    `KAESO_SOURCE_KEY="${secret}"`,
    'STATE="${TMPDIR:-/tmp}/kaeso-antigravity"',
    "hook=$(cat)",
    "printf '{}'",
    "exec > /dev/null 2>&1",
    "(",
    "  log=$(printf '%s' \"$hook\" | jq -r '.transcriptPath // empty')",
    "  conversation=$(printf '%s' \"$hook\" | jq -r '.conversationId // empty')",
    '  full="$(dirname "$log")/transcript_full.jsonl"',
    '  [ -n "$conversation" ] && [ -f "$full" ] || exit 0',
    '  mkdir -p "$STATE" && mark="$STATE/$conversation"',
    '  sent=$(cat "$mark" 2> /dev/null || echo -1)',
    `  body=$({ printf '%s\\n' "$hook"; cat "$full"; } | jq -cs --argjson sent "$sent" '.[0] as $hook | [.[1:][] | select(.step_index > $sent)] as $new | [$new[] | select(.type == "PLANNER_RESPONSE") | .step_index + range(1; 1 + (.tool_calls // [] | length))] as $results | {hook: ($hook${content ? "" : " | del(.lastUserInput)"}), steps: ([$new[] | if .type == "PLANNER_RESPONSE" then {${fields.join(", ")}, ${calls}} elif (.step_index as $i | any($results[]; . == $i)) then ${result} else empty end] | .[:1000])}')`,
    "  last=$(printf '%s' \"$body\" | jq '[.steps[].step_index] | max // empty')",
    '  [ -n "$last" ] || exit 0',
    '  printf \'%s\' "$body" | curl -fsS -m 30 "$KAESO_INGEST" -H "Authorization: Bearer $KAESO_SOURCE_KEY" \\',
    '    -H "Content-Type: application/json" --data-binary @- && printf \'%s\' "$last" > "$mark"',
    ") < /dev/null &",
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
        "# Add otlphttp/kaeso to the exporters of the pipelines you have already.",
        "receivers:",
        "  otlp:",
        "    protocols:",
        "      http:",
        "exporters:",
        "  otlphttp/kaeso:",
        `    endpoint: ${ingestUrl}/otlp`,
        "    headers:",
        `      Authorization: Bearer ${secret}`,
        "service:",
        "  pipelines:",
        "    traces:",
        "      receivers: [otlp]",
        "      exporters: [otlphttp/kaeso]",
        "    logs:",
        "      receivers: [otlp]",
        "      exporters: [otlphttp/kaeso]",
      ].join("\n"),
    },
  ];
}

/** The batch the HTTP source's example sends; the gateway's tests take the same body. */
export const HTTP_EXAMPLE = {
  requests: [
    {
      id: "req_0001",
      duration_ms: 1200,
      provider: "openai",
      model: "gpt-5.4-mini",
      status: 200,
      usage: { input_tokens: 1200, output_tokens: 300 },
      user_email: "ada@example.com",
    },
  ],
  tool_calls: [{ id: "call_0001", duration_ms: 80, server: "linear", tool: "list_issues", user_email: "ada@example.com" }],
};

function http({ ingestUrl, secret }: SetupTarget): CodeTab[] {
  return [
    {
      label: "curl",
      lang: "bash",
      code: [
        `curl ${ingestUrl}/v1/events \\`,
        `  -H "Authorization: Bearer ${secret}" \\`,
        '  -H "Content-Type: application/json" \\',
        `  -d '${JSON.stringify(HTTP_EXAMPLE)}'`,
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
        "# Batches of 50 stay well under Kaeso's 8 MiB a batch: DEFAULT_BATCH_SIZE=50 on the proxy.",
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
  grok,
  opentelemetry,
  http,
  litellm,
  openrouter,
};

/** The exact lines that make a source's client report to Kaeso, one tab per place they can go. */
export function setupSnippets(target: SetupTarget): CodeTab[] {
  return SNIPPETS[target.kind](target);
}
