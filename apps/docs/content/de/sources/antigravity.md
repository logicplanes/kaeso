---
title: Antigravity
description: Zählen Sie die Modellaufrufe von Antigravity aus seinen Transkripten, mit einem Hook-Skript, das sie nach jedem Durchlauf meldet.
order: 4
---

Antigravity hat keinen OpenTelemetry-Export, führt aber Hooks aus und speichert ein Transkript jeder Unterhaltung, mit den Tokens jedes Modellaufrufs. Ein kleines Hook-Skript sendet die neuen Aufrufe nach jedem Durchlauf an eine Kaeso-Quelle, und sie werden beobachtete Anfragen im Projekt.

[Fügen Sie eine Quelle](index.md#quelle-hinzufügen) der Art **Antigravity** hinzu. Speichern Sie dieses Skript als `~/.gemini/config/kaeso-source.sh`, mit der Adresse Ihrer Installation und dem Schlüssel der Quelle. Es braucht `curl` und `jq`:

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
  body=$({ printf '%s\n' "$hook"; cat "$full"; } | jq -cs --argjson sent "$sent" '.[0] as $hook | [.[1:][] | select(.step_index > $sent)] as $new | [$new[] | select(.type == "PLANNER_RESPONSE") | .step_index + range(1; 1 + (.tool_calls // [] | length))] as $results | {hook: ($hook | del(.lastUserInput)), steps: ([$new[] | if .type == "PLANNER_RESPONSE" then {type, step_index, created_at, model, input_tokens, cache_read_tokens, cache_write_tokens, cache_creation_input_tokens, output_tokens, tool_calls: (.tool_calls // [] | map({name}))} elif (.step_index as $i | any($results[]; . == $i)) then {step_index, type, status, created_at} else empty end] | .[:1000])}')
  last=$(printf '%s' "$body" | jq '[.steps[].step_index] | max // empty')
  [ -n "$last" ] || exit 0
  printf '%s' "$body" | curl -fsS -m 30 "$KAESO_INGEST" -H "Authorization: Bearer $KAESO_SOURCE_KEY" \
    -H "Content-Type: application/json" --data-binary @- && printf '%s' "$last" > "$mark"
) < /dev/null &
```

Tragen Sie dann den Hook in `~/.gemini/config/hooks.json` ein, oder in eine bestehende Datei:

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

Beginnen Sie eine Unterhaltung in Antigravity. Nach dem ersten Modellaufruf zeigt die Quelle **Verbunden**, und der Aufruf erscheint in den Anfrageprotokollen des Projekts.

## Wie der Hook arbeitet

Nach jedem Modelldurchlauf führt Antigravity das Skript aus und wartet auf seine Antwort. Das Skript antwortet sofort und sendet im Hintergrund, sodass kein Modellaufruf auf Kaeso wartet. Es liest die `transcript_full.jsonl` der Unterhaltung, neben dem Transkript, das Antigravity nennt, und sendet die Modellaufrufe seit der letzten Sendung, höchstens 1.000 Schritte auf einmal. Mit jedem Aufruf sendet es die Namen der Tools, die er aufgerufen hat, und die Schritte, die sie ausgeführt haben. Den zuletzt gesendeten merkt es sich je Unterhaltung unter `$TMPDIR/kaeso-antigravity`.

Ein Hook in der `.agents/hooks.json` eines Arbeitsbereichs meldet nur die Unterhaltungen dieses Arbeitsbereichs; der in `~/.gemini/config` meldet alle.

## Prompts behalten

Das Skript lässt Ihren letzten Prompt und den Text jedes Schritts heraus. Um sie zu behalten, schalten Sie **Prompts und Antworten behalten** für die Quelle ein und kopieren Sie das Skript erneut aus ihrer Detailansicht: Mit eingeschalteten Inhalten sendet es sie.

## Was Kaeso liest

Jeder Schritt `PLANNER_RESPONSE` des Transkripts wird eine Anfrage mit Modell, Eingabe-, gecachten und Ausgabe-Tokens und Zeitpunkt. Antigravity benennt seine Modelle mit der Effort-Stufe am Ende, etwa `gemini-3.8-flash-low`. Kennen der Katalog oder KPMR das Modell ohne sie, protokolliert und bepreist Kaeso die Anfrage als dieses Modell, behält die Stufe als Reasoning-Aufwand und ermittelt den Anbieter aus dem Modell, sodass jede Stufe als ein Modell erscheint. Ein Modell, das nur Antigravity kennt, etwa `gemini-4-neon-high`, behält seinen ganzen Namen und hat keine Kosten, zählt aber trotzdem seine Tokens. Das Transkript von Antigravity enthält keine Zeiten, deshalb haben seine Anfragen keine Dauer und bleiben aus Latenzwerten heraus.

Ein Aufruf eines MCP-Tools wird ein Tool-Aufruf, mit Server und Tool und ob der Schritt, der ihn ausgeführt hat, erfolgreich war. Ein Skript, das Sie kopiert haben, bevor Kaeso Tool-Aufrufe las, sendet keine; kopieren Sie es erneut aus der Detailansicht der Quelle.

## Nächste Schritte

- [Quellen](../concepts/sources.md): Wie Kaeso beobachtete Anfragen zählt und wem sie gehören.
- [Gemini CLI](gemini-cli.md): Zählen Sie die Anfragen und MCP-Tool-Aufrufe von Gemini CLI.
