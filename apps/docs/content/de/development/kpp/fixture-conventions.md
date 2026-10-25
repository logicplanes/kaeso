---
title: Fixture-Konventionen
nav: Fixture-Konventionen
description: Die Regeln, denen jede konstruierte Fixture folgt, damit die Codec-Tests sie exakt vergleichen können.
order: 10
---

Fixtures, die keine Live-Aufzeichnungen sind, folgen diesen Regeln, damit die Tests sie exakt vergleichen können:

- **Paare.** Eine Anfrage, eine Antwort und ein Stream mit demselben Basisnamen gehören zusammen. Der Stream ist dieselbe Antwort wie die Response und ergibt zusammengesetzt genau diese Response; Ausnahmen nennt die `README.md` des Dialektordners.
- **Zahlen, die übereinstimmen.** Summen sind die Summe ihrer Teile, und ein Stream meldet dieselbe Nutzung wie seine Antwort.
- **Erfundene Werte in der Live-Form.** IDs folgen dem Live-Format des Anbieters. Signaturen und verschlüsseltes Reasoning sind zufällig, aber wohlgeformt; ein Anbieter würde sie bei einer Wiederholung ablehnen.
- **Winzige, aber gültige Dateien.** Ein Bild ist ein 1×1-PNG, ein PDF eine Seite mit korrekter Querverweistabelle und Audio ein WAV-Header mit acht stillen Samples.
- **Formatierung.** JSON-Dateien sind mit zwei Leerzeichen eingerückt und schreiben Nicht-ASCII-Zeichen als UTF-8. SSE-Zeilen `data:` enthalten einzeiliges JSON, wie auf der Leitung.
- **Erfundenes nur in `unknown-*`.** Keine andere Fixture enthält ein Feld, einen Block oder ein Event, das der Anbieter nicht dokumentiert.

## Anthropic Messages

- Jede Anfrage setzt `anthropic-version: 2023-06-01` voraus, was nicht gespeichert wird. Eine `.headers.json` gibt es nur, wo eine Anfrage `anthropic-beta` braucht.
- IDs sehen aus wie `msg_01…`, `toolu_01…`, `srvtoolu_01…`, `file_011C…` und `req_011C…`.
- Eine `Message` enthält `model`, `id`, `type`, `role`, `content`, `stop_reason`, `stop_sequence`, `stop_details` und `usage`, wie es die aufgezeichneten Streams des SDK und die Dokumentation zeigen. `container` und `diagnostics` fehlen, weil sie nur bei Verwendung gesetzt sind.
- `usage` enthält `input_tokens`, die beiden Cache-Zahlen, die Aufschlüsselung `cache_creation`, `output_tokens`, `service_tier` und `inference_geo`. `output_tokens_details` erscheint nur nach Thinking und `server_tool_use` nur, wenn ein Server-Tool lief.
- `cache_creation_input_tokens` ist die Summe seiner Aufschlüsselung, und `thinking_tokens` ist höchstens `output_tokens`.
- `message_start` enthält die Nutzung der Antwort mit einem kleinen `output_tokens`, das letzte `message_delta` die kumulierten Zahlen.

## OpenAI Chat Completions

- Keine Anfrage braucht einen weitergeleiteten Header, daher gibt es keine `.headers.json`-Dateien.
- Ein Stream ist das, was seine Anfrage mit zusätzlichem `"stream": true` und `stream_options.include_usage` zurückgibt. Seine Chunks halten die Feldreihenfolge `id`, `object`, `created`, `model`, `service_tier`, `system_fingerprint`, `choices`, `usage`, `obfuscation` ein.
- Der erste Chunk enthält `delta.role` mit `content: ""`, mit `content: null` und dem Kopf des ersten Tool-Aufrufs oder mit `content: null, refusal: ""`. Auf das letzte Delta folgt ein Chunk mit `delta: {}`.
- Jeder Chunk außer dem letzten hat `"usage": null`, und der letzte, mit `"choices": []`, enthält `usage`. Jeder Chunk enthält `obfuscation`, zufälliges Padding, das die Spezifikation standardmäßig mitsendet.
- `prompt_tokens` enthält die gecachten und die Cache-Schreib-Token, `completion_tokens` die Reasoning-Token. `total_tokens` ist ihre Summe.

## OpenAI Responses

- Beispiele aus der Dokumentation verwenden das Modell der Spezifikation, `gpt-6-astra`. Gebaute Fixtures verwenden `gpt-5.5` mit Reasoning und `gpt-4.1` ohne, und ihre Antworten nennen die Snapshots `gpt-5.5-2026-04-23` und `gpt-4.1-2025-04-14`.
- IDs folgen `<prefix>_<16 hex>00<8 hex unix time><24 hex>`, mit den Präfixen `resp`, `msg`, `fc`, `rs` und `ws`, und die Items einer Antwort teilen ihr 16-Hex-Präfix. Aufruf-IDs sind `call_` und 24 Buchstaben und Ziffern.
- Gebaute `Response`-Objekte enthalten jedes Feld der obersten Ebene des aktuellen SDK, auch `access_programs`. Beispiele aus der Dokumentation behalten ihre kleineren Feldmengen, und der Codec führt beide verlustfrei hin und zurück.
- `input_tokens` enthält `cached_tokens`, und `output_tokens` enthält `reasoning_tokens`. `input_tokens_details.cache_write_tokens` ist immer vorhanden.
- In einem Stream ist `event:` gleich `data.type`, und `sequence_number` zählt ab 0 hoch. Die Deltas ergeben ihren `.done`-Text und die endgültige Antwort, und kein `[DONE]` folgt.
- Jedes `*.delta`-Event enthält `obfuscation`, außer in einem Stream, dessen Anfrage `include_obfuscation: false` setzt, und im eigenen Stream der Dokumentation.

## OpenAI Embeddings

- `input` deckt seine vier Formen ab: einen String, ein Array von Strings, ein Array von Token-IDs und ein Array von Token-ID-Arrays.
- Gebaute Anfragen setzen `dimensions: 8`, damit die Vektoren kurz bleiben. Es sind zufällige Einheitsvektoren, jeder Wert die kürzeste Dezimalzahl, die float32 übersteht.
- Eine `base64`-Antwort enthält Little-Endian-float32-Werte, und `usage.total_tokens` ist gleich `usage.prompt_tokens`.
