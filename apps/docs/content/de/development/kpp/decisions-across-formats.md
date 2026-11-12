---
title: Entscheidungen über Formate hinweg
nav: Entscheidungen
description: Wie Entscheidungsanfragen und Antworten zwischen der Decisions-API von OpenAI und der System-One-API von TypeSafe wechseln.
order: 3
---

Entscheidungsanfragen wechseln über ein `DecideRequest` zwischen der Decisions-API von OpenAI ([OpenAI Decisions](openai-decisions.md)) und der System-One-API von TypeSafe ([System One](system-one.md)). Ein Ziel lehnt ab, was sein Format nicht ausdrücken kann, bevor etwas gesendet wird. Einige benannte Anpassungen stehen im Header `kaeso-adapted` der Antwort, und Hinweise ohne Platz im Ziel in `kaeso-left-out`. Die Kennungen in Klammern sind die Entwurfsentscheidungen der Entscheidungsformate.

## Anfragen

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

## Antworten

Antworten wechseln nach Position: Der Encoder des App-Formats nimmt jeden Namen, jede Option und jede Stufe aus der Anfrage der App.

- **Konfidenz (D8).** Eine Antwort im eigenen Format behält ihre `confidence` Byte für Byte. Eine aus einem anderen Format bekommt sie aus den Wahrscheinlichkeiten neu berechnet: mit den Formeln von TypeSafe für System-One-Apps und ebenso für OpenAI-Apps, deren zwei dokumentierte Beispiele zu ihnen passen. Eine Ja-Nein-Antwort hat keine.
- **Verweigerungen (D9).** Eine Verweigerung von OpenAI erreicht eine System-One-App als `{"type": "refusal"}` unter dem Schlüssel der Frage. Die SDKs von TypeSafe überspringen Antworttypen, die sie nicht kennen, und die anderen Antworten zählen weiter.
- **Labels und Legenden.** Eine OpenAI-App bekommt das Label jeder Stufe aus ihrer Anfrage; eine System-One-App bekommt eine `legend` aus den Stufen ihrer Anfrage.
- **Verbrauch.** KPP zählt Eingabetokens ohne Cache. `input_tokens` von OpenAI enthält Cache-Lese- und Schreibvorgänge, daher zieht der Decoder sie ab, und der Encoder addiert sie wieder; `total_tokens` ist Eingabe plus Ausgabe.
