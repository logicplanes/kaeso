---
title: Sampling über Formate hinweg
nav: Sampling
description: Die Sampling-Einstellungen, die jedes API-Format annimmt, und die Werte, die Kaeso weglässt, weil sie nichts verlangen.
order: 3
---

Sampling-Einstellungen bestimmen, wie ein Modell seine Wörter wählt: Temperatur, Top P und Top K, die Penalties, ein Seed, Stoppsequenzen und das Ausgabelimit. Jedes API-Format nimmt eine andere Auswahl davon an. Diese Seite listet, was ein Ziel ablehnt, und die Werte, die Kaeso stattdessen weglässt, weil sie nichts verlangen.

## Einstellungen, die ein Ziel ablehnt

Sampling-Einstellungen, die ein Ziel nicht einhalten kann, sind `Unsupported` und werden nie weggelassen oder umskaliert, außer eine Penalty von 0 ([neutrale Werte](#neutrale-werte)):

| Ziel | Abgelehnt |
| --- | --- |
| Anthropic | `seed`, Frequency- und Presence-Penalties und eine `temperature` außerhalb von 0 bis 1, dem Bereich bei Anthropic |
| OpenAI Chat Completions | `top_k` und mehr als vier Stoppsequenzen |
| OpenAI Responses | `top_k`, `seed`, die Penalties und Stoppsequenzen, die es bei Responses nicht gibt, und ein Ausgabelimit unter 16 Token, dem Minimum von `CreateResponse` |
| Gemini API | Mehr als fünf Stoppsequenzen (`GenerationConfig.stopSequences`) |
| Amazon Bedrock Converse | `top_k`, `seed` und die Penalties, die in die eigenen Felder einer Modellfamilie gehören |

Zahlen behalten ihre Schreibweise innerhalb eines Dialekts: `temperature: 1` kommt als `1` zurück und `1.0` als `1.0`, solange der Wert unverändert ist. Über Dialekte hinweg gilt die eigene Schreibweise des Ziels, daher erreicht `temperature: 1` aus Chat Anthropic als `1.0`.

KPP hat kein `n`: Eine Chat-Anfrage mit `n` über 1 wird schon beim Dekodieren abgelehnt, unabhängig vom Ziel. `logprobs`, `logit_bias` und `top_logprobs` von Chat sind Extras, die jeder andere Dialekt ablehnt ([Extras](cross-format.md#extras)), außer sie haben ihren [neutralen Wert](#neutrale-werte).

## Neutrale Werte

Chat-Apps wie NextChat senden die Sampling-Felder von OpenAI mit jeder Anfrage, mit dem Wert, den die API ohne das Feld verwendet. Ein solcher Wert verlangt nichts. Wo das Ziel oder das Profil seines Providers das Feld ablehnen würde, lässt Kaeso es stattdessen weg und nennt es im Header `kaeso-left-out` so, wie die App es geschrieben hat. Jeder andere Wert wird wie bisher abgelehnt. Die Werte stehen in `cross::NEUTRAL_VALUES`, dazu kommen die Penalties.

| Feld | Wert | Weggelassen für |
| --- | --- | --- |
| `frequency_penalty`, `presence_penalty`, `generationConfig.frequencyPenalty` und `.presencePenalty` einer Gemini-App | `0` | Anthropic, Responses, Amazon Bedrock Converse und Chat-Provider, die sie ablehnen |
| `logprobs` | `false` | jedes andere Format und Chat-Provider, die es ablehnen |
| `top_logprobs` | `0` | ebenso |
| `logit_bias`, `metadata` | `{}` | ebenso |
| `store` | `false` | ebenso; ein Responses-Ziel erhält Kaesos eigenes `store: false`, dort wird es nicht genannt |
| `service_tier`, aus Chat und aus Anthropic | `"auto"` | ebenso, und Anthropic-Provider, die es ablehnen |
| `thread`, aus Anthropic | `null` | jedes andere Format; es verlangt keinen Nachrichten-Thread, wie wenn das Feld fehlt |
| `verbosity` | `"medium"` | ebenso |
| `modalities` | `["text"]` | Chat-Provider, die es ablehnen; andere Formate schreiben ohnehin Text |

Jeder Wert ist der `default` des Felds im OpenAPI-Dokument von OpenAI und in der API-Referenz von Anthropic; ein leeres `logit_bias` oder `metadata` bedeutet dasselbe wie ihr Default `null`. `service_tier: "default"` ist nicht neutral: Es verlangt Standardverarbeitung, wo die Einstellung eines Projekts eine andere Stufe wählen könnte. `web_search_options: {}` ebenso wenig: Damit sucht ein Suchmodell. Ein als `null` gesendetes Feld ist Form, die andere Formate nie erhalten; ein Chat-Provider, der das Feld ablehnt, erhält auch das `null` nicht.

Fixtures: `crates/kaeso-codecs/fixtures/openai_chat/requests/nextchat-defaults.json` (die Anfrage von NextChat), `crates/kaeso-codecs/fixtures/openai_chat/requests/neutral-defaults.json`.
