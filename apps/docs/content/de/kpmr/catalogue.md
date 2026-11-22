---
title: Katalogformat
description: Jedes Feld des KPMR-Katalogs im Format 1 und wie das Gateway es liest.
order: 2
---

Der Katalog ist ein JSON-Dokument in Kaesos eigenem Format, ausgeliefert als `application/vnd.kaeso.kpmr+json; version=1`. Diese Seite listet jedes Feld des Formats 1. Hier ist ein gekürzter Katalog mit einem Anbieter und einem Modell:

```json
{
  "format": 1,
  "revision": "2026-11-21T02:59:02Z-efb348278140",
  "generated_at": "2026-11-21T02:59:02Z",
  "refresh_seconds": 3600,
  "provider_ids": ["302ai", "alibaba", "anthropic"],
  "providers": {
    "anthropic": {
      "id": "anthropic",
      "name": "Anthropic",
      "docs_url": "https://docs.anthropic.com/en/docs/about-claude/models",
      "api_url": null,
      "env": ["ANTHROPIC_API_KEY"],
      "lines": {
        "claude-opus-latest": {
          "model": "claude-opus-5-5",
          "since": "2026-09-22",
          "previous": "claude-opus-5"
        }
      },
      "models": {
        "claude-opus-5-5": {
          "id": "claude-opus-5-5",
          "name": "Claude Opus 5.5",
          "family": "claude-opus",
          "kind": "generation",
          "status": "active",
          "release_date": "2026-09-22",
          "modalities": {
            "input": ["text", "image", "pdf"],
            "output": ["text"]
          },
          "limits": { "context": 1000000, "input": null, "output": 128000 },
          "pricing": {
            "unit": "usd_per_million_tokens",
            "input": "4",
            "output": "20",
            "cache_read": "0.2",
            "cache_write": "5",
            "cache_write_1h": "8",
            "tiers": []
          },
          "reasoning": {
            "efforts": ["low", "medium", "high", "xhigh", "max"],
            "adaptive_thinking": true,
            "thinking_binding": true
          },
          "decisions": null,
          "lifecycle": { "shutdown_on": null, "replacement": null },
          "gateway": { "egress_dialect": null, "router": false }
        }
      }
    }
  }
}
```

Vier Regeln gelten für das ganze Dokument:

- **Jeder Schlüssel ist vorhanden.** Ein unbekannter Wert ist `null`, nie `false`, sodass ein Leser „unbekannt“ von „nein“ unterscheiden kann. Das Beispiel oben lässt Schlüssel nur weg, um kurz zu bleiben.
- **Preise sind exakte Dezimalzahlen als Strings** in US-Dollar, nie Gleitkommazahlen. Addieren Sie sie mit einer Dezimal-Bibliothek.
- **Anbieter und Modelle sind Maps**, nach ID geschlüsselt und sortiert.
- **Unbekannte Schlüssel werden ignoriert.** Format 1 wächst durch neue Felder, und ein Leser ignoriert, was er nicht kennt. Eine Änderung, die einen Leser stören würde, wird Format 2, und KPMR liefert beide aus, solange Gateways umsteigen.

## Dokument

| Feld | Bedeutung |
| --- | --- |
| `format` | Die Formatversion, `1` |
| `revision` | Der Zeitpunkt, zu dem der Inhalt gebaut wurde, und ein Hash davon. Sie ändert sich, sobald sich der Inhalt ändert |
| `generated_at` | Wann dieser Inhalt zuerst veröffentlicht wurde, in UTC |
| `refresh_seconds` | Wie viele Sekunden bis zur nächsten Anfrage zu warten sind |
| `sources` | Woher die Daten kommen und unter welcher Lizenz, dazu `commit` und Zahl der `entries` der Registry |
| `provider_ids` | Die ID jedes Anbieters, den KPMR kennt, auch wenn Sie nur nach einigen gefragt haben. Ein [eigener Anbieter](../console/custom-providers.md) darf keine davon nehmen |
| `providers` | Die Anbieter, nach denen Sie gefragt haben, nach ID geschlüsselt. Eine ID, die KPMR nicht kennt, fehlt |

## Anbieter

| Feld | Bedeutung |
| --- | --- |
| `id` | Die Anbieter-ID, wie Kaeso sie nennt, etwa `anthropic` ([Unterstützte Anbieter](../concepts/supported-providers.md)) |
| `name` | Der Name des Anbieters |
| `docs_url` | Die Modelldokumentation des Anbieters |
| `api_url` | Die Basisadresse einer OpenAI-kompatiblen API, wo der Anbieter eine hat |
| `env` | Die Umgebungsvariablen, aus denen seine SDKs den Schlüssel lesen |
| `models` | Die Modelle des Anbieters, nach der ID geschlüsselt, die seine API annimmt |
| `lines` | Die Modelllinien des Anbieters, nach Namen geschlüsselt, etwa `claude-opus-latest`. Leer, wenn er keine hat |

### Linien

Jeder Eintrag in `lines` lässt einen Namen auf ein Modell des Anbieters zeigen ([Abschaltungen und Modelllinien](lifecycle-and-lines.md#modelllinien)):

| Feld | Bedeutung |
| --- | --- |
| `model` | Die Modell-ID, auf die die Linie jetzt zeigt |
| `since` | Wann dieses Modell erschien und die Linie übernahm; `null`, wenn unbekannt |
| `previous` | Das Modell, auf das die Linie vorher zeigte; `null` bei einer Linie, die bisher ein Modell hatte |

## Modelle

| Feld | Bedeutung |
| --- | --- |
| `id` | Die ID, die an die API des Anbieters geht, gleich dem Schlüssel des Modells |
| `name` | Der Anzeigename |
| `description` | Ein Satz dazu, wofür das Modell gedacht ist |
| `family` | Die Modellfamilie, etwa `claude-sonnet` |
| `kind` | Was das Modell tut: `generation`, `embedding`, `decision`, `image`, `video`, `speech`, `transcription`, `realtime`, `reranking`, `music` oder `sound_effects`. Lesen Sie eine Art, die Sie nicht kennen, als unbekannt |
| `status` | `active`, `preview`, `deprecated` oder `retired`; er folgt den Daten in `lifecycle`, und ein Modell kann `deprecated` sein, bevor ein Datum angekündigt ist |
| `release_date` | Wann das Modell erschien, `YYYY-MM-DD`, oder `YYYY-MM`, wo nur der Monat bekannt ist |
| `last_updated` | Wann das Modell zuletzt aktualisiert wurde, in derselben Form |
| `knowledge` | Der Stichtag der Trainingsdaten, `YYYY-MM` oder `YYYY-MM-DD` |
| `open_weights` | Ob die Gewichte veröffentlicht sind |
| `modalities` | `input` und `output`, jeweils eine Liste aus `text`, `image`, `audio`, `video` und `pdf`. `output` ist leer bei einem Modell, das nur entscheidet |
| `limits` | `context`, `input` und `output`, in Token |
| `pricing` | Die Preise des Modells ([Preise](#preise)) |
| `capabilities` | Was das Modell annimmt ([Fähigkeiten](#fähigkeiten)) |
| `reasoning` | `null` bei einem Modell ohne Reasoning; sonst wie es schlussfolgert ([Reasoning](#reasoning)) |
| `decisions` | Die Fragen, die ein Entscheidungsmodell beantwortet; `null` bei einem Modell, das nicht entscheidet ([Entscheidungen](#entscheidungen)) |
| `lifecycle` | Wann der Anbieter das Modell abschaltet ([Lebenszyklus](#lebenszyklus)) |
| `gateway` | Wie ein Gateway die Anfragen des Modells sendet ([Gateway](#gateway)) |

### Fähigkeiten

| Feld | `true`, wenn das Modell |
| --- | --- |
| `tool_call` | Tools aufruft |
| `forced_tool_choice` | Ein `tool_choice` annimmt, das einen Tool-Aufruf erzwingt. Bei `false` lehnt Kaeso eine erzwungene Wahl ab und verlangt `auto` |
| `structured_output` | In einem JSON-Schema antwortet, das die Anfrage vorgibt |
| `temperature` | `temperature` annimmt |
| `attachments` | Dateien annimmt, etwa Bilder oder PDFs |
| `reasoning` | Schlussfolgert, bevor es antwortet |
| `interleaved_reasoning` | Zwischen Tool-Aufrufen innerhalb einer Antwort schlussfolgert |

### Reasoning

| Feld | Bedeutung |
| --- | --- |
| `efforts` | Die Aufwandsstufen, die das Modell annimmt, etwa `low` und `high`; `[]` für keine, `null`, wenn unbekannt |
| `adaptive_thinking` | Das Modell nimmt adaptives Denken an, bei dem es selbst entscheidet, wie lange es nachdenkt |
| `thinking_binding` | Thinking-Blöcke müssen bei dem Zug bleiben, zu dem sie gehören |

### Entscheidungen

Ein Entscheidungsmodell beantwortet typisierte Fragen mit Wahrscheinlichkeiten, statt Text zu schreiben ([Decisions-API](../api/decisions.md)). Ein Modell, das nur entscheidet, hat `kind: "decision"`, etwa `gemini-decision-1`: Seine Ausgabemodalitäten sind leer, daher bietet ein Gateway, das keine Entscheidungen kennt, es nie für Chat an. Ein Modell, das auch Text schreibt, etwa `gpt-6-luna`, behält seine Art und hat zusätzlich `decisions`. Entscheidungsmodelle kosten ihre Eingabe-Tokens; ihr Ausgabepreis ist `0`.

| Feld | Bedeutung |
| --- | --- |
| `question_types` | `predicate` (ja oder nein), `choice` (eine von mehreren Optionen), `score` (eine Stufe auf einer geordneten Skala) oder `box` (wo sich etwas in einem Bild befindet) |
| `max_questions` | Die meisten Fragen, die eine Anfrage stellen darf |
| `max_options` | Die meisten Optionen, die eine Frage `choice` haben darf |
| `max_levels` | Die meisten Stufen, die eine Frage `score` haben darf |
| `max_images` | Die meisten Bilder, die eine Anfrage senden darf |

Jedes Feld `max_` ist `null`, wenn unbekannt.

### Lebenszyklus

Jeder Schlüssel ist vorhanden und `null`, bis der Anbieter ihn ankündigt. `status` wird an `deprecated_on` zu `deprecated` und an `shutdown_on` zu `retired`. Ein Modell kann auch `deprecated` sein, während jedes Datum noch `null` ist ([Abschaltungen und Modelllinien](lifecycle-and-lines.md)).

| Feld | Bedeutung |
| --- | --- |
| `announced_on` | Der Tag, an dem der Anbieter die Abschaltung angekündigt hat, `YYYY-MM-DD` |
| `deprecated_on` | Der Tag, ab dem das Modell veraltet ist: Es funktioniert noch, aber der Anbieter möchte, dass Apps umsteigen |
| `shutdown_on` | Der Tag, an dem der Anbieter das Modell abschaltet |
| `replacement` | Die ID des Modells desselben Anbieters, das er stattdessen empfiehlt |
| `source` | Die Seite, auf der der Anbieter es angekündigt hat |

### Gateway

| Feld | Bedeutung |
| --- | --- |
| `egress_dialect` | Das API-Format, in dem ein Gateway die Anfragen des Modells sendet, etwa `openai_responses`; `null` für das Standardformat des Anbieters ([APIs nach Anbieter](../concepts/provider-apis.md)) |
| `router` | `true` für einen Router, der für jede Anfrage ein anderes Modell wählt |

## Preise

`pricing` enthält Dezimalzahlen als Strings in US-Dollar je Million Token und `null`, wo ein Preis unbekannt ist.

| Feld | Bedeutung |
| --- | --- |
| `unit` | Immer `usd_per_million_tokens` |
| `input` | Eingabe-Tokens |
| `output` | Ausgabe-Tokens. Mit `output_image` nur die Textausgabe |
| `cache_read` | Eingabe, die aus dem Prompt-Cache gelesen wird |
| `cache_write` | Eingabe, die in den Cache geschrieben wird, mit fünf Minuten Lebensdauer |
| `cache_write_1h` | Eingabe, die in den Cache geschrieben wird, mit einer Stunde Lebensdauer, wo der Anbieter eine dokumentiert |
| `reasoning` | Reasoning-Tokens, wo sie getrennt von der Ausgabe kosten |
| `input_audio`, `output_audio` | Audio-Tokens der Eingabe und Ausgabe |
| `input_image`, `output_image` | Bild-Tokens der Eingabe und Ausgabe |
| `cache_read_image` | Bildeingabe, die aus dem Cache gelesen wird |
| `tiers` | Preise für lange Kontexte, das niedrigste `context_above` zuerst, oder leer ([Stufen für lange Kontexte](#stufen-für-lange-kontexte)) |
| `above_200k` | Eine ältere Form der Preise für lange Kontexte, für ältere Leser behalten und oft `null`. Lesen Sie `tiers`, das die genaue Schwelle hat |
| `per_unit` | Preise je Bild, Sekunde oder anderer Einheit, für Medienmodelle ([Preise je Einheit](#preise-je-einheit)) |

### Stufen für lange Kontexte

Die Preise einer Stufe gelten für jedes Token einer Anfrage, deren Prompt länger als `context_above` Token ist. Ein Preis `null` in einer Stufe ist der Grundpreis. Zum Beispiel kostet `gpt-5.4` von OpenAI ab 272.000 Token mehr:

```json wrap
{ "context_above": 272000, "input": "5", "output": "22.5", "cache_read": "0.5", "cache_write": null }
```

| Feld | Bedeutung |
| --- | --- |
| `context_above` | Die Prompt-Länge in Token, ab der die Stufe gilt |
| `input`, `output`, `cache_read`, `cache_write` | Die Preise der Stufe |

### Preise je Einheit

Medienmodelle listen Preise in `per_unit`: was ein Bild, eine Sekunde Video oder eine Million Zeichen kostet. Jeder Eintrag sagt, wofür er gilt; eine Bedingung `null` gilt für jeden Wert.

| Feld | Bedeutung |
| --- | --- |
| `per` | `image`, `megapixel`, `video`, `second`, `minute`, `hour`, `million_characters` oder `search` |
| `of` | Ob er zählt, was hineingeht, `input`, oder herauskommt, `output` |
| `usd` | Der Preis in US-Dollar, eine Dezimalzahl als String |
| `size` | Die Bildgröße, für die er gilt, etwa `1024x1024` |
| `quality` | Die Qualität, für die er gilt, etwa `high` |
| `resolution` | Die Auflösung, für die er gilt, etwa `1080p` |
| `duration` | Bei einem Preis je Video die Länge des Clips in Sekunden |
| `audio` | Bei Video, ob der Preis mit Ton gilt |
| `minimum_seconds` | Die kürzeste abgerechnete Länge für eine Zeiteinheit |
| `estimate` | `true`, wo der Anbieter nur eine Schätzung veröffentlicht |

## Wie das Gateway ihn liest

Das Gateway liest nur die Anbieter, die es aufruft, und prüft jedes Modell für sich:

- **Ein Modell, das es nicht lesen kann, überspringt es**, und der Rest des Katalogs gilt trotzdem. Das ist ein Modell, dessen Schlüssel von seiner `id` abweicht, dessen `pricing.unit` nicht `usd_per_million_tokens` ist oder dessen Preis keine Dezimalzahl von 0 bis 1.000.000 ist.
- **Eine Stufe oder eine Linie, die es nicht lesen kann, lässt es weg**, nie das Modell oder den Anbieter.
- **Einen Katalog in einem anderen Format** lehnt es ganz ab, und die verwendeten Daten bleiben.
- **Die Werte speisen den Katalog jedes Projekts** als Werte von KPMR, die ein Projekt überschreiben kann ([Modelle und Aliasse](../console/models-and-aliases.md#werte-eines-modells-ändern)).

## Nächste Schritte

- [KPMR-API](api.md): Rufen Sie den Katalog ab und prüfen Sie seine Signatur.
- [Abschaltungen und Modelllinien](lifecycle-and-lines.md): Sehen Sie, wie sich die Daten des Lebenszyklus und die Linien mit der Zeit ändern.
- [Modelldaten aktuell halten](sync.md): Legen Sie fest, wie oft das Gateway den Katalog abruft.
