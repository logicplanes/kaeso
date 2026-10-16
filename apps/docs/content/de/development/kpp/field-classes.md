---
title: Feldklassen
description: Jedes KPP-Feld als semantisch, also vom Ziel einzuhalten, oder als Hinweis, den das Ziel ignorieren darf.
order: 2
---

Jedes KPP-Feld ist entweder **semantisch** oder ein **Hinweis** (siehe die [Entwurfsprinzipien](index.md#entwurfsprinzipien)).

- **Semantisch:** Das Ziel muss das Feld einhalten. Kann der Zieldialekt oder das Modell es nicht ausdrücken, gibt `check()` des Codecs `Unsupported` zurück und nennt das Feld. Es wird nichts gesendet.
- **Hinweis:** Das Ziel darf das Feld ignorieren. Hat das Ziel keine Entsprechung, lässt der Codec es stillschweigend weg.

„Weggelassen“ heißt unten immer *nur dort, wo das Ziel das Feld nicht ausdrücken kann*; ein Ziel, das einen Hinweis ausdrücken kann, bekommt ihn immer.

## `GenerateRequest`

| Feld | Klasse | Anmerkungen |
| --- | --- | --- |
| `model` | semantisch | Wird vom Gateway vor dem Kodieren aufgelöst. |
| `system` | semantisch | |
| `messages` | semantisch | |
| `tools` (`Function`) | semantisch | |
| `tools` (`Unknown`) | semantisch | Nur im Ursprungsdialekt ausdrückbar. |
| `tool_choice` | semantisch | `Unknown` nur im Ursprungsdialekt ausdrückbar. |
| `parallel_tool_calls` | semantisch | |
| `output` | semantisch | `JsonObject` hat bei Anthropic keine Entsprechung und ist `Unsupported`, ohne Nachbildung ([strukturierte Ausgabe](cross-format.md#strukturierte-ausgabe)). |
| `reasoning.mode`, `.effort`, `.budget_tokens` | semantisch | Aufwand und Budget werden nach der Tabelle unter [Aufwand und Budget](cross-format.md#aufwand-und-budget) ineinander umgerechnet. |
| `reasoning.summary` | **Hinweis** | Steuert nur, ob Zusammenfassungen zurückkommen. Anthropic: `thinking.display: "summarized"` ([lesbares Reasoning und Zusammenfassungen](cross-format.md#lesbares-reasoning-und-zusammenfassungen)). |
| `reasoning.include_encrypted` | **Hinweis** | Steuert nur, ob verschlüsseltes Reasoning zurückkommt; nur OpenAI Responses ([lesbares Reasoning und Zusammenfassungen](cross-format.md#lesbares-reasoning-und-zusammenfassungen)). |
| `sampling.max_output_tokens` | semantisch | |
| `sampling.temperature`, `.top_p` | semantisch | |
| `sampling.top_k` | semantisch | Nur Anthropic; anderswo `Unsupported` ([Sampling](cross-format.md#sampling)). |
| `sampling.stop` | semantisch | |
| `sampling.seed` | semantisch | Nur OpenAI; für Anthropic `Unsupported` ([Sampling](cross-format.md#sampling)). |
| `sampling.frequency_penalty`, `.presence_penalty` | semantisch | Nur OpenAI; für Anthropic `Unsupported` ([Sampling](cross-format.md#sampling)). |
| `user` | **Hinweis** | Eine Endbenutzer-ID zur Missbrauchsüberwachung; sie ändert nie die Ausgabe. |
| `stream` | semantisch | |
| `extras.fields` | semantisch | Wird nur in den Ursprungsdialekt zurückgeschrieben; anderswo `Unsupported`, außer das Feld steht auf der [Extras-Allowlist](extras-allowlist.md). |
| `extras.form` | **Hinweis** | Form-Markierungen für die Darstellung; andere Dialekte ignorieren sie. |

## `Message`

| Feld | Klasse | Anmerkungen |
| --- | --- | --- |
| `role` | semantisch | Eine `System`-Nachricht nach dem Beginn der Unterhaltung ist für Anthropic `Unsupported`, außer die Anfrage kam aus Anthropic Messages ([Systemnachrichten](cross-format.md#systemnachrichten)). |
| `blocks` | semantisch | Die Reihenfolge der Blöcke bleibt erhalten. |
| `extras` | wie bei der Anfrage | |

## `Block`

| Variante / Feld | Klasse | Anmerkungen |
| --- | --- | --- |
| `Text.text` | semantisch | |
| `Text.citations` | semantisch | Roh und an den Ursprung gebunden; ein anderer Dialekt bekommt `Unsupported`. |
| `Text.cache`, `Image.cache`, `Document.cache`, `ToolCall.cache`, `ToolResult.cache` | **Hinweis** | Für OpenAI weggelassen, weil OpenAI selbst cacht ([Cache-Hinweise](cross-format.md#cache-hinweise)). |
| `Image.source` | semantisch | `FileId` gilt nur für den Ursprung. |
| `Document.source` | semantisch | Textquellen nur, wo das Ziel Textdokumente unterstützt. |
| `Document.title`, `.context` | semantisch | |
| `Document.citations` | semantisch | `Unsupported`, wo das Ziel keine Dokumente zitieren kann. |
| `ToolCall.*` | semantisch | IDs bleiben erhalten; lehnt ein Ziel die Zeichen einer ID ab, wird sie auf feste Weise umgeschrieben und auf dem Rückweg zurückübersetzt. |
| `ToolResult.call_id`, `.content` | semantisch | |
| `ToolResult.is_error` | **Hinweis** für Ziele ohne Fehler-Flag | Für OpenAI Chat und Responses weggelassen; der Inhalt trägt den Fehlertext ([Tool-Fehler](cross-format.md#tool-fehler)). |
| `Reasoning.summary`, `.text` | **Hinweis** | Lesbares Reasoning wird in der Anfrage eines anderen Dialekts weggelassen ([Reasoning](cross-format.md#reasoning)). In Antworten wird es zusammen mit dem Originalblock in einem [Reasoning-Blob](cross-format.md#reasoning-blobs) gezeigt. |
| `Reasoning.opaque` | semantisch für den Ursprung | Wird Byte für Byte an Ursprungsdialekt und -anbieter zurückgespielt; nie anderswohin gesendet, dort fällt der ganze Block weg ([Reasoning](cross-format.md#reasoning)). |
| `Reasoning.origin` | — | Entscheidet über das Zurückspielen ([Reasoning](cross-format.md#reasoning)). |
| `Refusal.text` | semantisch | |
| `Unknown` | semantisch | Nur im Ursprungsdialekt. |

## `CacheHint`

| Feld | Klasse | Anmerkungen |
| --- | --- | --- |
| `ttl` | **Hinweis** | |
| `extras` | **Hinweis** | Fällt mit dem Hinweis weg. |

## `EmbedRequest`

| Feld | Klasse | Anmerkungen |
| --- | --- | --- |
| `model`, `input`, `dimensions`, `encoding` | semantisch | |
| `user` | **Hinweis** | |
| `extras` | wie bei `GenerateRequest` | |

## Antworten und Stream-Events

Antworten und Stream-Events fließen von einem Anbieter zu einer App. Sie haben keine Hinweise: Ein Feld, das der Dialekt der App nicht ausdrücken kann, ist entweder ableitbar, etwa ein Stoppgrund, der auf die nächstliegende Entsprechung abgebildet wird, oder es wird für das Zurückspielen im selben Dialekt in den Extras mitgeführt. Die Mapping-Seite jedes Dialekts listet diese Abbildungen auf: [Anthropic Messages](anthropic-messages.md), [OpenAI Chat Completions](openai-chat.md), [OpenAI Responses](openai-responses.md).

Reasoning erreicht eine App eines anderen Dialekts, wo deren Format es tragen kann, und zwar so verpackt, dass es zurückkommen kann ([Reasoning-Blobs](cross-format.md#reasoning-blobs), `cross::reasoning_reply_policy`). Unbekannte Blöcke erreichen nur Apps ihres eigenen Dialekts. Alles andere bleibt für die App unsichtbar ([Antworten](cross-format.md#antworten), `cross::reply_policy`), in Streams ersetzt durch [Keepalive](cross-format.md#keepalives)-Frames. Das Gateway protokolliert die vollständige KPP-Antwort.
