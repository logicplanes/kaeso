---
title: Entscheidungen über Formate hinweg
nav: Entscheidungen
description: Wie Entscheidungsanfragen und Antworten zwischen den Decisions-APIs von OpenAI und Gemini und System One von TypeSafe wechseln.
order: 3
---

Entscheidungsanfragen wechseln über ein `DecideRequest` zwischen der Decisions-API von OpenAI ([OpenAI Decisions](openai-decisions.md)), der System-One-API von TypeSafe ([System One](system-one.md)) und der Decisions-API von Gemini ([Gemini Decisions](gemini-decisions.md)). Ein Ziel lehnt ab, was sein Format nicht ausdrücken kann, bevor etwas gesendet wird. Einige benannte Anpassungen stehen im Header `kaeso-adapted` der Antwort, und Hinweise ohne Platz im Ziel in `kaeso-left-out`. Die Kennungen in Klammern sind die Entwurfsentscheidungen der Entscheidungsformate.

## Anfragen zwischen OpenAI und System One

| Was | OpenAI-App zu System-One-Anbieter | System-One-App zu OpenAI |
| --- | --- | --- |
| Fragennamen (D19) | Bleiben, wenn jede Frage einen Namen hat, der nur einmal vorkommt und für den Anbieter gültig ist; sonst alle als `q0`…`qN-1`, `adapted: names` | Jeder Schlüssel wird der `name` der Frage |
| Boolesche Werte (D18) | Als `"true"` und `"false"` gesendet, `adapted: booleans`; eine Frage mit `true` und `"true"` ist `Unsupported` | — |
| Bewertungsstufen | `"label: description"`, wo eine Stufe beides hat, `adapted: levels`; sonst das Label allein | Der Text jeder Stufe wird ihr `label` |
| Strukturierter Zustand (D13) | — | `input_json` innerhalb der Grenzen von OpenAI; sonst `input_text` mit dem JSON, wie geschrieben, `adapted: state` |
| Kriterien von Ja-Nein-Fragen (D11) | — | Mit einer Vorlage in die Anweisungen eingefügt, `adapted: criteria` |
| JSON-Anweisungen und -Beschreibungen | — | Ihr JSON, wie geschrieben, als Text, `adapted: instructions` oder `criteria` |
| Gespräche (D12) | `Unsupported` (`conversation input`): senden Sie eine Benutzernachricht | — |
| Bilder | Data-URLs werden Assets; URLs sind `Unsupported` (`image URL`): senden Sie das Bild als Data-URL (D6) | Jede Referenz in einem String der Eingabe, einer Zustandsliste oder der Anweisungen wird an ihrer Stelle ein `input_image`-Teil; anderswo `Unsupported` |
| Box-Fragen | Nur, wo das Profil sie beantwortet | `Unsupported` |
| Grenzen (D5) | Die Grenzen des Profils für Fragen, Optionen, Stufen und Bilder | 400 Fragen, 2 bis 255 Optionen, 2 bis 10 Stufen |
| `prompt_cache_key`, `prompt_cache_options`, Breakpoints | Weggelassen, `left_out: cache` | — |
| `safety_identifier` | Weggelassen, `left_out: user` | — |
| `detail` | Weggelassen | — |

Die Vorlage für Kriterien fügt den Anweisungen eine Leerzeile und dann `It is true when: <true>` und `It is false when: <false>` an, je eine Zeile. Anweisungen aus Teilen bekommen die Zeilen als weiteren Textteil. JSON-Text wird nie neu formatiert.

## Anfragen von und zu Gemini

| Was | OpenAI- oder System-One-App zu Gemini | Gemini-App zu OpenAI oder System One |
| --- | --- | --- |
| Fragennamen (D19) | Bleiben, wenn jede Frage einen Namen hat, der nur einmal vorkommt und zu `[A-Za-z0-9_-]{1,64}` passt; sonst alle als `q0`…`qN-1`, `adapted: names` | Jeder Schlüssel wird der Name der Frage |
| Optionsschlüssel | Jeder Wert, boolesche Werte als `"true"` und `"false"` (`adapted: booleans`). Ein Schlüssel, den Google ablehnt, ist `Unsupported` (`option key`) | Jeder Schlüssel wird der Wert der Option |
| Optionsbeschreibungen | Eine fehlende Beschreibung ist `""` | Bleiben, auch `""` |
| Bewertungsstufen | Wie für System One: `"label: description"`, `adapted: levels`; eine Stufe ohne Text ist `Unsupported` | Der Text jeder Stufe, wie eine Stufe von System One |
| Strukturierter Zustand (D13) | Sein JSON, wie geschrieben, als String der Eingabe; keine Anpassung wird genannt, da Googles Leitfaden das verlangt | — |
| JSON-Teile, -Anweisungen und -Beschreibungen | Ihr JSON, wie geschrieben, als Text; `adapted: instructions` oder `criteria` | — |
| Kriterien von Ja-Nein-Fragen | `true_description` und `false_description` | In die Anweisungen von OpenAI eingefügt (D11); `true` und `false` von System One |
| Gespräche (D12) | `Unsupported` (`conversation input`) | — |
| Bilder | Bytes und Data-URLs als `data`; URLs mit `https` als `uri` mit dem MIME-Typ ihrer Endung; URLs mit `http`, `gs://` und von YouTube sind `Unsupported` | Eingebettete Bilder wechseln; URIs der Files API sind `Unsupported` (`file id`) |
| Asset-Referenzen | Im String der Eingabe oder in einem String einer Zustandsliste ein Block `image` an ihrer Stelle; der Text der Listenelemente mit einem Zeilenumbruch verbunden; anderswo `Unsupported` | — |
| Bilder in Anweisungen oder Beschreibungen | `Unsupported`: Gemini nimmt dort nur Text an | — |
| Audio, Video und PDF | — | `Unsupported`, mit dem Teil benannt |
| `detail` und `resolution` | `low` und `high` von OpenAI werden `resolution`; andere Werte werden weggelassen | `low` und `high` werden `detail` von OpenAI; andere Werte werden weggelassen |
| Box-Fragen | `Unsupported` | — |
| Grenzen (D5) | 1.000 Fragen, 2 bis 255 Optionen, 2 bis 255 Stufen | Wie in der Tabelle oben |
| `prompt_cache_key`, `prompt_cache_options`, `safety_identifier`, Breakpoints | Weggelassen, `left_out: cache` oder `user` | — |

Google lehnt einen Optionsschlüssel ab, der mehr als 128 Zeichen oder 512 Bytes hat, an einem Ende ein Leerzeichen, ein Steuerzeichen oder ein Leerzeichen außerhalb von ASCII. Gemini verbindet benachbarte Textblöcke ohne Trennzeichen, daher erreichen seine Blöcke System One als Zustandsliste von Strings und OpenAI als Textteile.

## Antworten

Antworten wechseln nach Position: Der Encoder des App-Formats nimmt jeden Namen, jede Option und jede Stufe aus der Anfrage der App.

- **Konfidenz (D8).** Eine Antwort im eigenen Format behält ihre `confidence` Byte für Byte. Eine aus einem anderen Format bekommt sie aus den Wahrscheinlichkeiten neu berechnet: mit den Formeln von TypeSafe für System-One-Apps und ebenso für OpenAI-Apps, deren zwei dokumentierte Beispiele zu ihnen passen. Gemini-Apps bekommen Googles Regel: die höchste Wahrscheinlichkeit, und `max(p, 1 − p)` für eine Ja-Nein-Antwort. Für OpenAI- und System-One-Apps hat eine Ja-Nein-Antwort keine.
- **Verweigerungen (D9).** Eine Verweigerung von OpenAI erreicht eine System-One-App als `{"type": "refusal"}` unter dem Schlüssel der Frage. Die SDKs von TypeSafe überspringen Antworttypen, die sie nicht kennen, und die anderen Antworten zählen weiter. Das Format von Gemini hat keine Verweigerung als Antwort, daher erreicht eine Antwort mit Verweigerungen eine Gemini-App als `ContentFiltered`, ein 400 mit dem Code `safety`, das die verweigerten Fragen nennt.
- **Sicherheitssperren (D10).** Gemini meldet eine blockierte Eingabe als 400 mit einem Sicherheitscode oder als Antwort mit `prompt_feedback.block_reason` und ohne Antworten. Beides erreicht jede App als `ContentFiltered` in ihrem eigenen Format.
- **Labels und Legenden.** Eine OpenAI-App bekommt das Label jeder Stufe aus ihrer Anfrage; System-One- und Gemini-Apps bekommen eine `legend` aus den Stufen ihrer Anfrage.
- **Modell.** Antworten von Gemini nennen kein Modell, daher nimmt der Decoder das der Anfrage; Gemini-Apps bekommen keins.
- **Verbrauch.** KPP zählt Eingabetokens ohne Cache. `input_tokens` von OpenAI enthält Cache-Lese- und Schreibvorgänge, daher zieht der Decoder sie ab, und der Encoder addiert sie wieder; `total_tokens` ist Eingabe plus Ausgabe. Gemini zählt die Eingabe nach Modalität: KPP hält Bildtokens, Video und PDF-Seiten eingeschlossen, und Audiotokens getrennt. Eine Gemini-App bekommt `input_tokens_by_modality` mit Text als Rest.
