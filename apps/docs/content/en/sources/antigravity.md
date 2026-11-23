---
title: Antigravity
description: Count the model calls Antigravity makes, from its transcripts, with a hook script that reports them after each invocation.
order: 4
---

Antigravity has no OpenTelemetry export, but it runs hooks and keeps a transcript of every conversation, with each model call's tokens. A small hook script sends the calls it added to a Kaeso source after each invocation, and they become observed requests in the project.

[Add a source](index.md#add-a-source) of the kind **Antigravity**. Save this script as `~/.gemini/config/kaeso-source.sh`, with your install's address and the source's key. It needs `curl` and `jq`:

```bash
KAESO_INGEST="https://kaeso.example.com/ingest/v1/antigravity"
KAESO_SOURCE_KEY="sk-kso-src-…"
STATE="${TMPDIR:-/tmp}/kaeso-antigravity"
hook=$(cat)
printf '{}'
exec > /dev/null 2>&1
(
  log=$(printf '%s' "$hook" | jq -r '.transcriptPath // empty')
  conversation=$(printf '%s' "$hook" | jq -r '.conversationId // empty')
  full="$(dirname "$log")/transcript_full.jsonl"
  [ -n "$conversation" ] && [ -f "$full" ] || exit 0
  mkdir -p "$STATE" && mark="$STATE/$conversation"
  sent=$(cat "$mark" 2> /dev/null || echo -1)
  body=$({ printf '%s\n' "$hook"; cat "$full"; } | jq -cs --argjson sent "$sent" '.[0] as $hook | {hook: ($hook | del(.lastUserInput)), steps: ([.[1:][] | select(.type == "PLANNER_RESPONSE" and .step_index > $sent) | {type, step_index, created_at, model, input_tokens, cache_read_tokens, cache_write_tokens, cache_creation_input_tokens, output_tokens}] | .[:1000])}')
  last=$(printf '%s' "$body" | jq '[.steps[].step_index] | max // empty')
  [ -n "$last" ] || exit 0
  printf '%s' "$body" | curl -fsS -m 30 "$KAESO_INGEST" -H "Authorization: Bearer $KAESO_SOURCE_KEY" \
    -H "Content-Type: application/json" --data-binary @- && printf '%s' "$last" > "$mark"
) < /dev/null &
```

Then add the hook to `~/.gemini/config/hooks.json`, or to an existing one:

```json
{
  "kaeso": {
    "PostInvocation": [
      {
        "type": "command",
        "command": "sh ~/.gemini/config/kaeso-source.sh",
        "timeout": 15
      }
    ]
  }
}
```

Start a conversation in Antigravity. The source shows **Connected** after the first model call, and the call appears in the project's request logs.

## How the hook works

After each model invocation, Antigravity runs the script and waits for its answer. The script answers at once and sends in the background, so no model call waits for Kaeso. It reads the conversation's `transcript_full.jsonl`, next to the transcript Antigravity names, and sends the model calls added since it last sent, at most 1,000 at a time. It remembers the last one per conversation under `$TMPDIR/kaeso-antigravity`.

A hook in a workspace's `.agents/hooks.json` reports only that workspace's conversations; the one in `~/.gemini/config` reports all of them.

## Keep the prompts

The script leaves your last prompt and each step's text out. To keep them, turn on **Keep prompts and replies** for the source, then copy the script again from the source's sheet: with content on, it sends them.

## What Kaeso reads

Each `PLANNER_RESPONSE` step of the transcript becomes a request with its model, input, cached and output tokens, and its time. Antigravity names its models by internal ids, such as `gemini-4-neon-high`, which no catalogue prices, so these requests have no cost. They still count their tokens and toward token limits.

## Next steps

- [Sources](../concepts/sources.md): How Kaeso counts observed requests, and whose they are.
- [Gemini CLI](gemini-cli.md): Count Gemini CLI's requests and MCP tool calls.
