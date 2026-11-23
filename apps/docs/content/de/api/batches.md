---
title: Batches-API
description: Viele Anfragen günstiger ausführen, mit OpenAIs Batch- und Files-APIs, innerhalb Ihrer Schlüssel und Limits.
order: 5
---

Ein Batch führt viele Anfragen an ein Modell beim Anbieter innerhalb von 24 Stunden aus, zu einem niedrigeren Preis als Live-Anfragen. Kaeso bedient OpenAIs Batch- und Files-APIs, sodass der Batch-Ablauf des OpenAI-SDKs unverändert funktioniert: eine Datei mit Anfragen hochladen, einen Batch anlegen, nach seinem Stand fragen und die Ergebnisse herunterladen ([Batch API](https://developers.openai.com/api/docs/guides/batch)).

## Welche Modelle

Ein Batch läuft bei einem Anbieter, dessen Batch-API OpenAIs Batch-Format annimmt. Heute ist das OpenAI, zum halben Live-Preis. Ein Batch nennt ein Modell und einen Endpunkt, wie OpenAI es verlangt: `/v1/chat/completions`, `/v1/responses` oder `/v1/embeddings`. Kaeso übersetzt Batches noch nicht zwischen Formaten, also muss jede Zeile in einem Format sein, das der Anbieter des Modells annimmt.

## Einen Batch ausführen

```python
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key="sk-kso-...")

batch_file = client.files.create(file=open("requests.jsonl", "rb"), purpose="batch")
batch = client.batches.create(
    input_file_id=batch_file.id,
    endpoint="/v1/chat/completions",
    completion_window="24h",
)
batch = client.batches.retrieve(batch.id)
if batch.status == "completed":
    results = client.files.content(batch.output_file_id).text
```

Jede Zeile der Datei ist eine Anfrage: `{"custom_id": "request-1", "method": "POST", "url": "/v1/chat/completions", "body": {"model": "gpt-5", "messages": [...]}}`.

## Was Kaeso prüft

Beim Hochladen liest Kaeso jede Zeile so, wie der genannte Endpunkt eine Live-Anfrage liest, bevor etwas den Anbieter erreicht:

- Jede Zeile nennt denselben Endpunkt und dasselbe Modell, mit einer eigenen `custom_id` und `method` `POST`, und keinen Stream.
- Das Modell wird wie bei einer Live-Anfrage aufgelöst, nach Name, `provider/model` oder ID, und der Schlüssel muss es nutzen dürfen. Ein Alias wird abgelehnt: Ein Batch nennt ein Modell.
- Der Anbieter des Modells führt Batches aus, und der Schlüssel des Projekts für ihn ist ein API-Schlüssel.

Eine Zeile, die eine Regel verletzt, lehnt die ganze Datei ab, mit ihrer Zeilennummer. Kaeso sendet die Datei mit dem Modell jeder Zeile so weiter, wie der Anbieter es nennt, sodass ein Modell mit abweichender **Modell-ID beim Anbieter** unter dieser ID läuft. Die Datei muss in das Body-Limit der Modell-API passen (`body_limits.model_api`).

| Methode und Pfad | Was er tut |
| --- | --- |
| `POST /v1/files` | Lädt eine Batch-Eingabedatei hoch (`purpose` `batch`) |
| `GET /v1/files/{file_id}`, `DELETE /v1/files/{file_id}` | Liest oder löscht eine Datei |
| `GET /v1/files/{file_id}/content` | Lädt eine Datei herunter: eine Eingabedatei oder die Ausgabe oder Fehler eines Batches |
| `POST /v1/batches`, `GET /v1/batches` | Legt einen Batch an oder listet die Batches des Projekts, neueste zuerst (`after`, `limit`) |
| `GET /v1/batches/{batch_id}`, `POST /v1/batches/{batch_id}/cancel` | Liest einen Batch und fragt dabei den Anbieter nach seinem Stand, oder bricht ihn ab |

Dateien und Batches haben Kaeso-IDs und antworten nur Schlüsseln des Projekts, das sie angelegt hat; für jeden anderen Schlüssel gibt es sie nicht. Sie bleiben beim Anbieter unter dem Anbieter-Schlüssel, der sie angelegt hat: Nutzt das Projekt einen anderen Schlüssel für den Anbieter, erreicht Kaeso sie nicht mehr.

## Kosten, Limits und Protokolle

- Das Hochladen einer Datei und das Anlegen eines Batches zählen je als eine Anfrage gegen die Raten des Schlüssels. Die Zeilen zählen nicht gegen Raten pro Minute; das Anlegen eines Batches prüft und reserviert seine Höchstzahl an Tokens und die Ausgaben zum Batch-Preis gegen harte Token- und Ausgabenbudgets, bis der Batch endet, und lehnt Funktionen ab, die ein Ausgabenbudget nicht bepreisen kann.
- Endet ein Batch, liest Kaeso seine Ausgabe einmal und verbucht jede Zeile als Anfrage an das Modell des Batches, mit der Nutzung des Anbieters zum Batch-Preis. Zeilen, die beim Anbieter scheiterten, werden mit ihrem Status und ohne Nutzung verbucht. Für jede Zeile entsteht ein Eintrag im Anfrageprotokoll.
- Kaeso fragt den Anbieter alle fünf Minuten nach offenen Batches, und immer, wenn eine App nach einem Batch fragt.
