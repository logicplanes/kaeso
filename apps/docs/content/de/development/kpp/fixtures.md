---
title: Fixtures und Aufzeichnungen
nav: Fixtures
description: Die echten Payloads, gegen die die Codecs getestet werden, und wie neue bei den Anbietern aufgezeichnet werden.
order: 10
---

Jede Codec-Änderung beginnt mit einer Fixture: einem echten Anfrage-Body, einer Antwort, einem Stream oder einem Fehler in `crates/kaeso-codecs/fixtures`. Jede Fixture durchläuft einen Round Trip durch ihren eigenen Codec, und die Cross-Dialekt-Matrix schickt jede Generierungs-Fixture durch jeden anderen Dialekt. Die [SDK-Konformität](../testing.md#sdk-konformität) liefert die Fixtures an die offiziellen SDKs aus, der Mock-Anbieter des Gateways antwortet mit ihnen, und die Fuzz-Targets gehen von ihnen aus.

## Aufbau

| Ordner | Dialekt |
| --- | --- |
| `anthropic_messages/` | Anthropic Messages (`/v1/messages`, `/v1/messages/count_tokens`) |
| `openai_chat/` | OpenAI Chat Completions (`/v1/chat/completions`) |
| `openai_responses/` | OpenAI Responses (`/v1/responses`, `/v1/responses/input_tokens`) |
| `openai_embeddings/` | OpenAI Embeddings (`/v1/embeddings`) |
| `gemini/` | Gemini API (`{models}/{model}:generateContent`, `:streamGenerateContent`, `:countTokens`) |
| `gemini_embeddings/` | Gemini-Embeddings (`{models}/{model}:embedContent`, `:batchEmbedContents`) |
| `openai_images/` | OpenAI Images (`/v1/images/generations`, `/v1/images/edits`); Uploads als `requests/<name>.multipart` mit dem Content-Type in `<name>.meta.json` |
| `minimax_images/` | Bildgenerierung von MiniMax (`/v1/image_generation`) |
| `openai_speech/` | Speech von OpenAI (`/v1/audio/speech`); die Audioantwort als `responses/<name>.mp3` mit ihrem Content-Type in `<name>.mp3.meta.json` |
| `openai_transcriptions/` | Transkriptionen und Übersetzungen von OpenAI (`/v1/audio/transcriptions`, `/v1/audio/translations`); Uploads als `requests/<name>.multipart`, Textformen als `.txt`, `.srt` und `.vtt` |
| `minimax_speech/` | Text to Speech von MiniMax (`/v1/t2a_v2`) |
| `elevenlabs/` | ElevenLabs; Pfad und Query jeder Anfrage in `<name>.meta.json`, Audioantworten als `.mp3` und `.pcm`, der Stream mit Zeitmarken als `.ndjson` |

Jeder Dialektordner hat `requests/`, `responses/`, `streams/`, sofern der Endpunkt streamt, und `errors/`.

| Datei | Inhalt |
| --- | --- |
| `requests/<name>.json` | Der exakte Anfrage-Body |
| `requests/<name>.headers.json` | Weitergegebene Header von der Allowlist, die mit dieser Anfrage gesendet wurden |
| `requests/<name>.meta.json` | Nur Gemini: das `model` und die Methode (`action`, etwa `streamGenerateContent`), die der Pfad dieser Anfrage nennt, wo sie nicht `--model` und `generateContent` sind (`embedContent` bei Embeddings) |
| `responses/<name>.json` | Der exakte Body einer 200-Antwort |
| `streams/<name>.sse` | Die exakten SSE-Bytes |
| `errors/<name>.json` und `errors/<name>.meta.json` | Ein Fehler-Body und `{"status": …, "headers": {…}}`. Antwortet der Codec Apps mit einem anderen Body, etwa der OpenAI-Fehlerform für den String-`error` von SpaceX, enthält `"encoded"` diesen Body, und der Round-Trip vergleicht mit ihm |

Einige Endpunkte haben eigene Unterordner: `count_tokens/` für Anthropic und Gemini und `input_tokens/` für OpenAI Responses, jeweils in `requests/` und `responses/`, und `models/` für Modelllisten (`anthropic_messages/models/`, `gemini/models/` und `openai_chat/models/`, das alle drei OpenAI-Codecs nutzen).

Aufzeichnungen anderer Anbieter, die einen Dialekt sprechen, etwa einer OpenAI-kompatiblen API, kommen nach `<dialect>/providers/<id>/`, benannt nach der ID des Anbieters in `crates/kaeso-core/src/provider.rs`. Dieser Ordner hat dieselben Unterordner `requests/`, `responses/`, `streams/` und `errors/`, jeweils nur, wenn der Anbieter dort Dateien hat. `requests/` enthält Anfragen, die nur dieser Anbieter annimmt; jeder Name darin muss sich von den Anfrage-Fixtures des Dialekts unterscheiden.

Die Round-Trip-Tests, die Decoder-Tests und die Cross-Dialekt-Matrix lesen jeden Anbieterordner zusammen mit den Fixtures des Dialekts und benennen seine Dateien `providers/<id>/<name>`. Eine neue Aufzeichnung eines Anbieters ergänzt deshalb die Snapshots der Matrix; prüfen Sie sie und übernehmen Sie sie mit `INSTA_UPDATE=always cargo test -p kaeso-codecs --test cross_matrix`. Die SDK-Konformität liefert nur die Fixtures des Dialekts selbst aus; der Mock-Anbieter des Gateways liefert die Datei eines Anbieters aus, wenn ein Test sie als `providers/<id>/<name>` nennt.

- `unknown-*`-Fixtures tragen absichtlich erfundene Felder, Blöcke oder Events, um die Behandlung von `Extras` und `Unknown` zu testen.
- Fixtures, die ein Codec ablehnen muss (`stateful-*`, `n-2*`), werden statt auf einen Round Trip auf ihren `Unsupported`-Fehler getestet.

## Herkunft

Jeder Dialektordner und seine Ordner `requests/`, `responses/`, `streams/`, `errors/` und `models/` haben eine `README.md`, die jede Datei auflistet, was sie abdeckt und woher sie stammt:

| Kennzeichen | Quelle |
| --- | --- |
| `doc:` | Ein Beispiel aus der Dokumentation des Anbieters |
| `sdk:` | Ein Test oder eine Aufzeichnung aus einem offiziellen SDK |
| `schema:` | Ein Body, der aus den festgelegten OpenAPI-Schemas gebaut wurde |
| `RECORDED.md` | Eine Live-Aufzeichnung mit `cargo xtask record`, mit Modell und Zeitpunkt |

Die `README.md` eines Dialektordners nennt außerdem die festgelegten Spezifikationen und SDK-Versionen, denen seine Fixtures folgen. Seine Fixtures folgen den [Konventionen](#konventionen) unten. Ergänzen Sie dort mit jeder neuen Fixture eine Zeile.

Ein Anbieterordner hat eine eigene `README.md`. Sie enthält:

- Name und ID des Anbieters;
- seine API-Referenz, mit dem Datum, an dem Sie sie gelesen haben;
- die Basis-URL und die Modelle der Aufzeichnung;
- eine Zeile für jede Datei in seinen Unterordnern.

Die `RECORDED.md` jedes Unterordners nennt den Anbieter in ihren ersten Zeilen.

## Konventionen

Konstruierte Fixtures folgen den [Fixture-Konventionen](fixture-conventions.md), damit die Tests sie exakt vergleichen können.

## Live-Antworten aufzeichnen

`cargo xtask record` sendet Anfrage-Fixtures an den echten Anbieter und speichert, was zurückkommt. Es läuft nur auf ausdrücklichen Aufruf, nur auf Ihrem Rechner und nie in der CI und braucht den API-Schlüssel des Anbieters in der Umgebung:

```bash
export ANTHROPIC_API_KEY=…
cargo xtask record anthropic_messages tools-single thinking-enabled-budget
cargo xtask record anthropic_messages --count --all
```

```bash
export OPENAI_API_KEY=…
cargo xtask record openai_chat --all --dry-run
cargo xtask record openai_responses --count --all
```

Gemini-Fixtures werden bei Google AI Studio, dem Anbieter `google`, mit dem Schlüssel aus `GOOGLE_API_KEY` aufgezeichnet. Eine Gemini-Anfrage nennt ihr Modell im Pfad, also übergeben Sie `--model` oder tragen das Modell in die `.meta.json` der Fixture ein:

```bash
export GOOGLE_API_KEY=…
cargo xtask record gemini --model gemini-3.5-flash text-simple stream-text
cargo xtask record gemini_embeddings --model gemini-embedding-2 batch
```

Jeder andere Anbieter in `crates/kaeso-core/src/provider.rs` wird genauso mit `--provider` aufgezeichnet, meist mit `--model`, weil die Anfrage-Fixtures Modelle von OpenAI und Anthropic nennen:

```bash
export <ID>_API_KEY=…
cargo xtask record openai_chat --provider <id> --model <model> --dry-run \
  tools-single stream-include-usage
cargo xtask record openai_chat --provider <id> --model <model> \
  tools-single stream-include-usage
```

| Option | Wirkung |
| --- | --- |
| `<dialect>` | `anthropic_messages`, `openai_chat`, `openai_responses`, `openai_embeddings`, `gemini` oder `gemini_embeddings` |
| `<names>` | Die zu sendenden Anfrage-Fixtures, ohne `.json` |
| `--all` | Jede Anfrage-Fixture des Dialekts außer `stateful-*`, `unknown-*`, `n-2*` und `input-audio*`, die ein Anbieter ablehnen würde oder die erfundene Felder tragen |
| `--count` | Der Endpunkt zur Token-Zählung und seine Fixtures in `requests/count_tokens/` oder `requests/input_tokens/`; nur `anthropic_messages`, `openai_responses` und `gemini` und nur für einen Anbieter, dessen `count_tokens` dieser Dialekt ist |
| `--dry-run` | Gibt aus, was gesendet würde, und sendet nichts |
| `--provider <id>` | Der aufzuzeichnende Anbieter, nach seiner ID; er muss den Dialekt in seinen `dialects` führen. Standardmäßig `anthropic` für `anthropic_messages`, `google` (Google AI Studio, die einzige Quelle von `gemini` und `gemini_embeddings`) und `openai` für die übrigen |
| `--model <id>` | Sendet dieses Modell statt des Modells, das die jeweilige Anfrage-Fixture nennt, und trägt es in `RECORDED.md` ein; bei Gemini hat es Vorrang vor dem Modell in `.meta.json` |

- Es liest `requests/<name>.json`, für einen anderen Anbieter auch `providers/<id>/requests/<name>.json`, und, falls vorhanden, `<name>.headers.json`, dessen Header von der Allowlist (`anthropic-beta`, `anthropic-version`, `openai-beta`) mitgesendet werden. Anthropic-Anfragen tragen `anthropic-version: 2023-06-01`, außer diese Datei nennt eine andere Version.
- Es schreibt eine 2xx-Antwort nach `responses/recorded-<name>.json`, mit `--count` nach `responses/count_tokens/` oder `responses/input_tokens/`. Eine Anfrage mit `"stream": true` oder eine Gemini-Anfrage an `streamGenerateContent` ergibt `streams/recorded-<name>.sse`, die rohen Bytes, wie empfangen. Ein Fehler ergibt `errors/recorded-<name>.json` (`.txt`, wenn der Body kein JSON ist) und `recorded-<name>.meta.json` mit dem Status und einem etwaigen `retry-after`. Jede Aufzeichnung fügt in `RECORDED.md` dieses Ordners eine Zeile mit dem Modell und der UTC-Zeit hinzu. `openai`, `anthropic` und `google` zeichnen in die Ordner des Dialekts auf, jeder andere Anbieter nach `providers/<id>/`. JSON-Bodies werden mit zwei Leerzeichen eingerückt und behalten die Schlüsselreihenfolge des Anbieters.
- Der Schlüssel kommt aus `<ID>_API_KEY`, der ID des Anbieters in Großbuchstaben mit Unterstrichen statt Bindestrichen: `OPENAI_API_KEY`, `ANTHROPIC_API_KEY` und `FIREWORKS_AI_API_KEY` für einen Anbieter `fireworks-ai`. `<ID>_BASE_URL` ersetzt die Standard-Basis-URL des Anbieters, die auch das Gateway verwendet. Der Schlüssel kommt in den Auth-Header des Anbieters: `authorization: Bearer`, `x-api-key` oder `x-goog-api-key` für Google AI Studio.
- API-Schlüssel werden aus der Umgebung gelesen und nie geschrieben, und Anfrage-Header werden nie gespeichert.
- Bevor etwas geschrieben wird, entfernt es den Query-String jeder URL, die Zugangsdaten einer Cloud-Signatur trägt: AWS Signature Version 4, signierte URLs von Google Cloud und Shared Access Signatures von Azure. Solche vorsignierten Links kann eine Websuche liefern. Alles andere bleibt Byte für Byte erhalten.

Der Code steht in `crates/xtask/src/record.rs`. Prüfen Sie jede aufgezeichnete Datei vor dem Commit: Sie enthält, was immer das Modell gesagt hat, und darf keine personenbezogenen Daten enthalten. Die Round-Trip-Tests nehmen aufgezeichnete Fixtures von selbst auf.
