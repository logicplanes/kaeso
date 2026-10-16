---
title: Extras-Allowlist
description: Die Extras, die beim Formatwechsel einer Anfrage weggelassen statt abgelehnt werden, und warum das jeweils sicher ist.
order: 3
---

Extras aus Dialekt A werden nur in Dialekt A zurückgeschrieben. Geht eine Anfrage mit Extras an einen anderen Dialekt, gibt `check()` des Codecs `Unsupported` zurück und nennt das Feld, außer es steht hier: Dann wird es stillschweigend weggelassen ([Extras](cross-format.md#extras)).

Ein Eintrag ist nur erlaubt, wenn bekannt ist, dass das Feld keinen Einfluss auf die Ausgabe des Modells hat. Jeder Eintrag braucht eine Begründung in einer Zeile und die Fixture, die zeigt, dass ein echter Client das Feld sendet.

Einträge haben einen Geltungsbereich (`cross::EXTRAS_ALLOWLIST`): Ein Schlüssel ist nur dort erlaubt, wo er steht. Die Geltungsbereiche sind `request`, `message`, `tool`, `output`, `reasoning` und die Blockarten `text`, `image`, `document`, `tool_call`, `tool_result` und `refusal`. Header sind Schlüssel `header:<name>` im Bereich `request`.

Die Fixture-Pfade auf dieser Seite sind relativ zu `crates/kaeso-codecs/fixtures/`.

## Einträge

| Ursprungsdialekt | Bereich | Schlüssel | Begründung | Fixture / Client |
| --- | --- | --- | --- | --- |
| `anthropic_messages` | request | `header:anthropic-beta` | Beta-Header schalten nur Funktionen des Anbieters frei. Eine Funktion, die eine Anfrage wirklich nutzt, erscheint als Feld oder Block im Body und wird dort für sich geprüft. | Claude Code sendet bei jeder Anfrage `claude-code-20250219,interleaved-thinking-…` und weitere Betas; `anthropic_messages/requests/claude-code-mid-conversation-system.headers.json`, `anthropic_messages/requests/thinking-interleaved-tools.headers.json` |
| `anthropic_messages` | request | `header:anthropic-version` | Versionierung des Anthropic-Wire-Formats; anderswo bedeutungslos. Ein Wert gleich der festgelegten Version wird gar nicht gespeichert ([die Anthropic-Version](cross-format.md#die-anthropic-version)). | Anthropic-SDKs |
| `openai_chat`, `openai_responses`, `openai_embeddings` | request | `header:openai-beta` | Wie `anthropic-beta`. | Codex sendet `OpenAI-Beta: responses=experimental` |
| `anthropic_messages` | request | `cache_control` | Automatisches Prompt-Caching auf oberster Ebene. Cache-Hinweise sind Hinweise ([Cache-Hinweise](cross-format.md#cache-hinweise)). | `anthropic_messages/requests/system-string-multi-turn.json`, `explicit-nulls.json` |
| `anthropic_messages` | reasoning | `display` (`thinking.display`) | Steuert nur, ob Thinking-Text zurückkommt (`summarized` oder `omitted`). OpenAI Responses bekommt stattdessen `reasoning.summary: "auto"`, außer der Wert ist `omitted` ([lesbares Reasoning und Zusammenfassungen](cross-format.md#lesbares-reasoning-und-zusammenfassungen)). | `anthropic_messages/requests/thinking-display-omitted.json`, `thinking-adaptive-effort.json` |
| `openai_chat` | image | `image_url.detail` | Hinweis zur Bildauflösung (`low`, `high`, `auto`): Er ändert die Token-Kosten des Bildes, nicht die Frage; Ziele ohne ihn wählen ihre Auflösung selbst. | `openai_chat/requests/image-base64-detail.json` |
| `openai_chat` | document | `file.filename` | Der Dateiname, den OpenAI neben eingebettetem `file_data` verlangt. Metadaten, keine Anweisung. | `openai_chat/requests/file-pdf-data.json` |
| `openai_chat` | request | `prompt_cache_key` | Leitet Anfragen zu einem Prompt-Cache. Ein Cache-Hinweis ([Cache-Hinweise](cross-format.md#cache-hinweise)). | `openai_chat/requests/all-params-sdk.json` |
| `openai_responses` | request | `prompt_cache_key` | Wie oben. | `openai_responses/requests/sampling-and-ids.json` |
| `openai_responses` | image | `detail` (an `input_image`; `auto` ist bereits eine Form-Markierung) | Hinweis zur Bildauflösung, wie `image_url.detail` bei Chat. `input_file.detail` (PDF-Darstellung) steht **nicht** auf der Allowlist. | `openai_responses/requests/image-base64-file-id.json` |

## Bedingte Einträge

Diese Felder wechseln das Format nur mit den Werten unten (`cross::is_droppable_value` und die Regel pro Nachricht in `cross::check_foreign`); jeder andere Wert bleibt `Unsupported`.

| Ursprungsdialekt | Bereich | Schlüssel | Bedingung und Begründung | Fixture / Client |
| --- | --- | --- | --- | --- |
| `anthropic_messages` | request | `context_management` | Enthält nur `edits`, und jeder Edit ist `clear_thinking_*`. Das Löschen von Thinking-Blöcken entscheidet nur, welche früheren Anthropic-`thinking`-Blöcke das Modell sieht, und diese erreichen nie einen anderen Dialekt ([Reasoning](cross-format.md#reasoning)). | Claude Code 2.1.296 sendet bei jeder Anfrage `{"edits": [{"type": "clear_thinking_20251015", "keep": "all"}]}`; `anthropic_messages/requests/claude-code-mid-conversation-system.json` |
| `anthropic_messages` | message (`role: "system"`) | `output_config` | `{}` oder nur `effort`, gleich dem `output_config.effort` der Anfrage. Den Aufwand der Anfrage zu wiederholen ändert nichts. | Claude Code 2.1.296 wiederholt den Aufwand der Anfrage in seiner Systemnachricht mitten in der Unterhaltung; dieselbe Fixture |
