---
title: Fixtures und Aufzeichnungen
description: Die echten Payloads, gegen die die Codecs getestet werden, und wie neue bei den Anbietern aufgezeichnet werden.
order: 8
---

Jede Codec-Änderung beginnt mit einer Fixture: einem echten Anfrage-Body, einer Antwort, einem Stream oder einem Fehler in `crates/kaeso-codecs/fixtures`. Jede Fixture durchläuft einen Round Trip durch ihren eigenen Codec, die Cross-Dialekt-Matrix schickt jede Generierungs-Fixture durch jeden anderen Dialekt, die [SDK-Konformitätssuite](../testing.md#sdk-konformität) liefert sie an die offiziellen SDKs aus, der Mock-Anbieter des Gateways antwortet mit ihnen, und die Fuzz-Targets gehen von ihnen aus.

## Aufbau

| Ordner | Dialekt |
| --- | --- |
| `anthropic_messages/` | Anthropic Messages (`/v1/messages`, `/v1/messages/count_tokens`) |
| `openai_chat/` | OpenAI Chat Completions (`/v1/chat/completions`) |
| `openai_responses/` | OpenAI Responses (`/v1/responses`, `/v1/responses/input_tokens`) |
| `openai_embeddings/` | OpenAI Embeddings (`/v1/embeddings`) |

Jeder Dialektordner hat `requests/`, `responses/`, `streams/`, sofern der Endpunkt streamt, und `errors/`.

| Datei | Inhalt |
| --- | --- |
| `requests/<name>.json` | Der exakte Anfrage-Body |
| `requests/<name>.headers.json` | Weitergegebene Header von der Allowlist, die mit dieser Anfrage gesendet wurden |
| `responses/<name>.json` | Der exakte Body einer 200-Antwort |
| `streams/<name>.sse` | Die exakten SSE-Bytes |
| `errors/<name>.json` und `errors/<name>.meta.json` | Ein Fehler-Body und `{"status": …, "headers": {…}}` |

Einige Endpunkte haben eigene Unterordner: `count_tokens/` für Anthropic und `input_tokens/` für OpenAI Responses, jeweils in `requests/` und `responses/`, und `models/` für Modelllisten (`anthropic_messages/models/` und `openai_chat/models/`, das alle drei OpenAI-Codecs nutzen).

- `unknown-*`-Fixtures tragen absichtlich erfundene Felder, Blöcke oder Events, um die Behandlung von `Extras` und `Unknown` zu testen.
- Fixtures, die ein Codec ablehnen muss (`stateful-*`, `n-2*`, `input-audio*`), werden statt auf einen Round Trip auf ihren `Unsupported`-Fehler getestet.

## Herkunft

Jeder Dialektordner und seine Ordner `requests/`, `responses/`, `streams/`, `errors/` und `models/` haben eine `README.md`, die jede Datei auflistet, was sie abdeckt und woher sie stammt:

| Kennzeichen | Quelle |
| --- | --- |
| `doc:` | Ein Beispiel aus der Dokumentation des Anbieters |
| `sdk:` | Ein Test oder eine Aufzeichnung aus einem offiziellen SDK |
| `schema:` | Ein Body, der aus den festgelegten OpenAPI-Schemas gebaut wurde |
| `RECORDED.md` | Eine Live-Aufzeichnung mit `cargo xtask record`, mit Modell und Zeitpunkt |

Die `README.md` eines Dialektordners nennt außerdem die festgelegten Spezifikationen und SDK-Versionen, denen seine Fixtures folgen, und die Konventionen, die sie teilen, etwa IDs und Token-Zahlen, die zwischen einem Stream und seiner Antwort übereinstimmen. Ergänzen Sie dort mit jeder neuen Fixture eine Zeile.

## Live-Antworten aufzeichnen

`cargo xtask record` sendet Anfrage-Fixtures an den echten Anbieter und speichert, was zurückkommt. Es ist opt-in, läuft nur auf Ihrem Rechner und nie in der CI und braucht den API-Schlüssel des Anbieters in der Umgebung:

```bash
export ANTHROPIC_API_KEY=…
cargo xtask record anthropic_messages tools-single thinking-signature
cargo xtask record anthropic_messages --count --all
```

```bash
export OPENAI_API_KEY=…
cargo xtask record openai_chat --all --dry-run
cargo xtask record openai_responses --count --all
```

| Option | Wirkung |
| --- | --- |
| `<dialect>` | `anthropic_messages`, `openai_chat`, `openai_responses` oder `openai_embeddings` |
| `<names>` | Die zu sendenden Anfrage-Fixtures, ohne `.json` |
| `--all` | Jede Anfrage-Fixture des Dialekts außer `stateful-*`, `unknown-*`, `n-2*` und `input-audio*`, die ein Anbieter ablehnen würde oder die erfundene Felder tragen |
| `--count` | Der Endpunkt zur Token-Zählung und seine Fixtures in `requests/count_tokens/` oder `requests/input_tokens/`; nur `anthropic_messages` und `openai_responses` |
| `--dry-run` | Gibt aus, was gesendet würde, und sendet nichts |

- Es liest `requests/<name>.json` und, falls vorhanden, `<name>.headers.json`, dessen Header von der Allowlist (`anthropic-beta`, `anthropic-version`, `openai-beta`) mitgesendet werden. Anthropic-Anfragen tragen `anthropic-version: 2023-06-01`, außer diese Datei nennt eine andere Version.
- Es schreibt `responses/recorded-<name>.json` bei einer 2xx-Antwort (mit `--count` in `responses/count_tokens/` oder `responses/input_tokens/`), `streams/recorded-<name>.sse`, wenn die Anfrage `"stream": true` enthält (die rohen Bytes, wie empfangen), oder bei einem Fehler `errors/recorded-<name>.json` (`.txt`, wenn der Body kein JSON ist) mit `recorded-<name>.meta.json` (der Status und ein etwaiges `retry-after`), und fügt in `RECORDED.md` dieses Ordners eine Zeile mit dem Modell und der UTC-Zeit hinzu. JSON-Bodies werden mit zwei Leerzeichen eingerückt und behalten die Schlüsselreihenfolge des Anbieters.
- `ANTHROPIC_BASE_URL` und `OPENAI_BASE_URL` ändern die API-Adressen.
- API-Schlüssel werden aus der Umgebung gelesen und nie geschrieben, und Anfrage-Header werden nie gespeichert.
- Bevor etwas geschrieben wird, entfernt es den Query-String jeder URL, die Zugangsdaten einer Cloud-Signatur trägt (AWS Signature Version 4, signierte URLs von Google Cloud, Shared Access Signatures von Azure), etwa bei vorsignierten Links aus einer Websuche; alles andere bleibt Byte für Byte erhalten.

Der Code steht in `crates/xtask/src/record.rs`. Prüfen Sie jede aufgezeichnete Datei vor dem Commit: Sie enthält, was immer das Modell gesagt hat, und darf keine personenbezogenen Daten enthalten. Die Round-Trip-Tests nehmen aufgezeichnete Fixtures von selbst auf.
