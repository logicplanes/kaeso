---
title: AssemblyAI
description: Wie die Transkriptionsanfragen von KPP auf die Sync API von AssemblyAI abgebildet werden.
order: 8
---

Hier sehen Sie, wie eine Transkriptionsanfrage AssemblyAI erreicht: welches Feld wohin geht, was Kaeso ablehnt, die Grenzen der Sync API und woher jede Abbildung stammt.

- **Codec:** `kaeso_codecs::assemblyai::AssemblyAi` (`crates/kaeso-codecs/src/assemblyai/`)
- **Pfad:** `POST /v1/transcribe` auf `https://sync.assemblyai.com` (oder `https://sync.eu.assemblyai.com`), mit dem Schlüssel als `Authorization: Bearer` und dem Modell in `X-AAI-Model`
- **Fixtures:** `crates/kaeso-codecs/fixtures/assemblyai/`, das Beispiel des OpenAPI-Dokuments und aus seinem Schema gebaute Antworten, mit live aufgezeichnetem abgelehntem Schlüssel und einer Anfrage ohne Modell

**Quellen.** Die [Referenz der Sync API](https://www.assemblyai.com/docs/api-reference/sync-api/transcribe) von AssemblyAI und ihr OpenAPI-Dokument, gelesen am 14.11.2026.

## Grenzen

Die Sync API antwortet in derselben Anfrage. Sie nimmt Audio von 80 Millisekunden bis 20 Minuten und bis zu 256 MB und antwortet innerhalb von 30 Sekunden, sonst mit 504 `Inference Timeout`, das die App als Zeitüberschreitung erreicht. Längere Aufnahmen brauchen die asynchrone API von AssemblyAI, die Kaeso nicht aufruft.

## Anfrage

| KPP | Sync API |
| --- | --- |
| `file` | der Teil `audio`, mit dem Inhaltstyp des Uploads |
| `model` | der Header `X-AAI-Model` (`universal-3-6-pro`) |
| `language` | `config.language_codes`, sein primäres Subtag, für die 32 Sprachen, auf die die API hinsteuert; eine andere Sprache wird abgelehnt, und ohne Sprache transkribiert die API ohne Steuerung |
| `prompt` | `config.prompt`, höchstens 6000 Zeichen; AssemblyAI fügt ihn den Anweisungen des Modells hinzu |
| `verbose_json`, `srt`, `vtt`, Wort-Zeitstempel | `config.timestamps: true`; Kaeso schreibt Segmente und Untertitel aus den zeitlich markierten Wörtern |

Der Teil `config` entfällt, wenn er leer wäre. Kaeso lehnt vor dem Senden `temperature`, Segment-Zeitstempel, Sprecherangaben (`diarized_json`), Übersetzungen, Streaming und Audio per Link ab: Die Sync API hat keinen Platz dafür.

## Ergebnis

| Sync API | KPP |
| --- | --- |
| `text` | der Text |
| `audio_duration_ms` | die Länge des Audios, nach der abgerechnet wird |
| `words[]` `{text, start, end}` in Millisekunden | zeitlich markierte Wörter; ein Wort, das die API nicht ausrichten konnte, kommt ohne Zeiten, und dann bleiben die Wörter, wie AssemblyAI sie geschrieben hat |
| `words[].confidence`, `confidence`, `session_id`, `request_time_ms` | Extras |

Die Sync API meldet keine Sprache und keine Segmente.

## Fehler

| Sync API | KPP |
| --- | --- |
| `{"status", "title", "detail"}` (Problem Details) | `detail` als Meldung, die Art aus dem Status; `Bad Audio`, `Audio Too Short`, `Audio Too Large` und `Unsupported Media Type` nennen `file` |
| 404 `Invalid API key` | ein Authentifizierungsfehler: die API beantwortet einen abgelehnten Schlüssel mit 404, nicht mit dem dokumentierten 401 |
| reiner Text, etwa `Not found` ohne `X-AAI-Model` | der Text, die Art aus dem Status |

## Modellprüfung

Die Sync API listet keine Modelle und kennt keine Anfrage, die einen Schlüssel ohne Transkription prüft, daher antwortet **Verfügbarkeit prüfen**, dass sie keinen prüfen kann; die erste Transkription zeigt, ob der Schlüssel funktioniert.
