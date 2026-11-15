---
title: Rerank-API
description: Dokumente im Rerank-Format von Cohere danach ordnen, wie gut sie eine Anfrage beantworten, bei jedem Anbieter mit Rerank in Kaeso.
order: 12
---

Apps senden eine Suchanfrage und eine Liste von Dokumenten, und ein Rerank-Modell antwortet mit der Relevanz jedes Dokuments, das beste zuerst. Retrieval-Pipelines wählen damit die Abschnitte aus, die sie einem Chat-Modell geben. Kaeso nimmt das Rerank-Format von Cohere an, das das Cohere-SDK, LangChain, LlamaIndex und die meisten Rerank-Clients senden, und ruft die eigene Rerank-API jedes Anbieters auf.

```bash tab="curl"
curl https://kaeso.example.com/v1/rerank \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "rerank-v5.0-fast", "query": "Which plan includes priority support?", "documents": ["Starter includes email support.", "Business includes priority support by phone and chat.", "All plans include the community forum."], "top_n": 2}'
```

```python tab="Cohere SDK"
import os
import cohere

client = cohere.ClientV2(api_key=os.environ["KAESO_API_KEY"], base_url="https://kaeso.example.com")

reply = client.rerank(
    model="rerank-v5.0-fast",
    query="Which plan includes priority support?",
    documents=[
        "Starter includes email support.",
        "Business includes priority support by phone and chat.",
        "All plans include the community forum.",
    ],
    top_n=2,
)
for result in reply.results:
    print(result.index, result.relevance_score)
```

```ts tab="Cohere SDK (TypeScript)"
import { CohereClientV2 } from "cohere-ai";

const client = new CohereClientV2({ token: process.env.KAESO_API_KEY, environment: "https://kaeso.example.com" });

const reply = await client.rerank({
  model: "rerank-v5.0-fast",
  query: "Which plan includes priority support?",
  documents: ["Starter includes email support.", "Business includes priority support by phone and chat.", "All plans include the community forum."],
  topN: 2,
});
console.log(reply.results);
```

Die Antwort nennt die Dokumente mit ihrer Position in der Anfrage:

```json
{
  "results": [
    { "index": 1, "relevance_score": 0.93 },
    { "index": 0, "relevance_score": 0.19 }
  ],
  "id": "1a6f2c4e-0000-0000-0000-000000000000",
  "meta": { "api_version": { "version": "2" }, "billed_units": { "input_tokens": 61 } }
}
```

## Endpunkt

| Endpunkt | Format | Schlüssel | Body |
| --- | --- | --- | --- |
| `POST /v1/rerank`, `POST /v2/rerank` | Rerank-Format von Cohere | `Authorization: Bearer` | JSON: `model`, `query`, `documents`, `top_n`, `max_tokens_per_doc`, `return_documents` |

Beide Pfade nehmen denselben Body an: An `/v2/rerank` sendet ihn das Cohere-SDK, an `/v1/rerank` die meisten anderen Clients. Rerank-Antworten werden nie gestreamt, und der Body ist wie bei Chat-Anfragen durch [`body_limits.model_api`](../self-hosting/configuration.md#anfrage--und-antwortkörper) begrenzt. Eine Anfrage enthält höchstens 10.000 Dokumente; Anbieter nehmen weniger, und Cohere empfiehlt höchstens 1.000.

| Feld | Bedeutung |
| --- | --- |
| `model` | Ein beliebiger [Modellname](model-api.md#modellnamen) eines Rerank-Modells oder ein Alias. |
| `query` | Die Anfrage, an der die Dokumente gemessen werden. |
| `documents` | Die zu bewertenden Texte, als Strings oder als `{"text": "…"}`-Objekte. |
| `top_n` | Wie viele der besten Dokumente die Antwort nennt; alle, wenn das Feld fehlt. |
| `max_tokens_per_doc` | Wo der Anbieter jedes Dokument abschneidet. |
| `return_documents` | Ob jedes Ergebnis sein Dokument als `{"text": "…"}` enthält; `false`, wenn das Feld fehlt. Das v2-Format von Cohere hat kein solches Feld, daher ergänzt Kaeso die Dokumente aus der Anfrage, wenn der Anbieter sie weglässt. |

Felder von Cohere selbst, etwa `relevant_spans` (Context Focus) und `priority`, gehen unverändert an Cohere, und die `relevant_spans` der Antwort kommen zurück. Ein Anbieter, der für eines davon keinen Platz hat, lehnt die Anfrage ab, bevor etwas gesendet wird.

Schlüssel, Modellauflösung mit Aliassen und Routen, Limits und Budgets, Anfrage-Logs und Fehler im Format von Cohere, `{"id", "message"}`, funktionieren wie bei jeder anderen Modellanfrage.

## Anbieter

| Anbieter | Gesendetes Format | Pfad | Modelle |
| --- | --- | --- | --- |
| Cohere | Cohere Rerank v2 | `/v2/rerank`, an der Wurzel von Coheres Host neben seiner Kompatibilitäts-API | Rerank 5, `rerank-v5.0-pro` und `rerank-v5.0-fast` |

Ein Modell ist ein Rerank-Modell, wenn [KPMR](../concepts/kpmr.md) ihm die Art `reranking` gibt. Rerank-Modelle eines Anbieters, dessen Rerank-API Kaeso noch nicht aufruft, etwa die von NVIDIA, werden abgelehnt, bevor etwas gesendet wird: „Kaeso calls no rerank models on NVIDIA, so `nvidia/rerank-qa-mistral-4b` can't rerank documents.“

## Was abgelehnt wird

Kaeso lehnt vor dem Senden ab, mit einem 400 im Format von Cohere:

- ein Generierungs- oder Embeddings-Modell: „The model `command-a-03-2025` is a generation model; it cannot rerank documents.“;
- eine leere `documents`-Liste, ein `top_n` oder `max_tokens_per_doc` von 0 oder ein Dokument, das weder ein String noch ein Objekt mit `text` ist;
- ein Dokument mit Feldern neben `text` für einen Anbieter, der nur Text annimmt, etwa Cohere.

Ein Rerank-Modell lehnt Chat, Token-Zählungen und Embeddings ab: „The model `rerank-v5.0-fast` is a rerank model; send it to `/v1/rerank`.“

## Bewertungen, Kosten, Cache und Logs

- **Bewertungen** sind die des Anbieters und werden unverändert weitergegeben. Die von Cohere liegen zwischen 0 und 1; andere Anbieter dokumentieren keinen Bereich, vergleichen Sie Bewertungen daher nicht über Anbieter hinweg.
- **Kosten.** Eine Rerank-Anfrage kostet die Tokens, die der Anbieter abrechnet, zum Eingabepreis des Modells. Rerank 5 von Cohere meldet sie als `billed_units.input_tokens`. Ältere Cohere-Modelle rechnen Sucheinheiten ab, die Kaeso protokolliert, aber nicht bepreisen kann. Ein Modell ohne Eingabepreis kostet **Unbekannt**, und [Budgets](../console/limits-and-budgets.md) lehnen es ab. Budgets reservieren die Suchanfrage einmal pro Dokument, wie die Anbieter sie abrechnen.
- **Antwort-Cache.** Der [Antwort-Cache](../console/response-cache.md) beantwortet eine wiederholte Rerank-Anfrage wie eine wiederholte Chat-Anfrage.
- **Logs.** Das [Anfrage-Log](../console/request-logs.md) zeigt den Endpunkt `rerank`, die Anfrage und die bewerteten Ergebnisse.

## Nächste Schritte

- [Cohere Rerank](../development/kpp/cohere-rerank.md): Sehen, wie jedes Feld auf KPP abgebildet wird.
- [Modelle und Aliasse](../console/models-and-aliases.md): Rerank-Modelle hinzufügen und zwischen ihnen routen.
- [APIs nach Anbieter](../concepts/provider-apis.md#rerank): Sehen, mit welchen Anbietern Kaeso neu ordnet.
