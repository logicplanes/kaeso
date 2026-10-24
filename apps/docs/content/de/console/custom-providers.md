---
title: Eigene Anbieter
description: Fügen Sie einen eigenen Server, etwa ein selbst betriebenes vLLM oder einen internen Proxy, als Anbieter Ihrer Organisation hinzu.
order: 4
---

Ein eigener Anbieter ist ein Server, den Ihre Organisation selbst hinzufügt, etwa ein selbst betriebenes vLLM, Ollama, LM Studio, TGI oder SGLang oder ein interner Proxy. Kaeso ruft ihn in einem API-Format auf, das es schon übersetzt. Apps erreichen seine Modelle wie die jedes [unterstützten Anbieters](../concepts/supported-providers.md), aus jedem Format und nach denselben Regeln.

## Eigenen Anbieter hinzufügen

1. Öffnen Sie unter **Anbieter** den Reiter **Eigene Anbieter** und wählen Sie **Eigenen Anbieter hinzufügen**.
2. Geben Sie ihm einen **Name**, etwa `Research vLLM`. Die **ID** folgt dem Namen, bis Sie sie ändern. Modellnamen beginnen mit ihr, etwa `research-vllm/llama-3.3-70b`, und sie ändert sich nie.
3. Geben Sie die **Basis-URL** ein, etwa `https://vllm.example.com`.
4. Wählen Sie das **API-Format** und ob er **Liefert auch Embeddings** oder **Zählt Tokens** ([Format wählen](#format-wählen)).
5. Wählen Sie, wie er seinen **Schlüssel** nimmt: **Bearer-Token**, **Header x-api-key** oder **Kein Schlüssel** für einen Server in Ihrem eigenen Netz.

Fügen Sie ihm dann unter **Anbieter-Schlüssel** einen Schlüssel hinzu wie jedem Anbieter, außer er nimmt keinen ([Anbieter-Schlüssel](provider-keys.md)). Zuletzt fügen Sie seine Modelle einem Projekt hinzu ([seine Modelle hinzufügen](#seine-modelle-hinzufügen)).

Eine ID besteht aus Kleinbuchstaben und Ziffern mit einzelnen Bindestrichen dazwischen, höchstens 40 Zeichen. Sie darf nicht die ID eines eingebauten Anbieters sein und auch nicht die eines Anbieters auf [models.dev](https://models.dev), denn Kaeso könnte diesen Anbieter später aufnehmen. Jede Organisation hat ihre eigenen IDs, zwei Organisationen können also beide ein `research-vllm` haben.

Kaeso hängt den Pfad des Formats an die Basis-URL an, etwa `/v1/chat/completions`, wie bei jedem Anbieter. Ein Pfad in der Basis-URL bleibt: `https://proxy.example.com/llm` erhält Anfragen unter `https://proxy.example.com/llm/v1/chat/completions`. Ein Server, dessen API nicht unter `/v1` liegt, lässt sich noch nicht hinzufügen.

## Format wählen

Wählen Sie das Format, in dem Ihr Server antwortet:

| Ihr Server | API-Format |
| --- | --- |
| vLLM, Ollama, LM Studio, TGI, SGLang und die meisten OpenAI-kompatiblen Server | OpenAI Chat Completions |
| Ein Server oder Proxy mit OpenAIs `POST /v1/responses` | OpenAI Responses |
| Ein Server oder Proxy mit Anthropics `POST /v1/messages` | Anthropic Messages |

- **Liefert auch Embeddings:** Schalten Sie es ein, wenn der Server `POST /v1/embeddings` im Format von OpenAI beantwortet. Seine Embedding-Modelle bedienen dann `/v1/embeddings`.
- **Zählt Tokens:** Nur Responses und Messages haben einen Endpunkt für Token-Zählungen, `/v1/responses/input_tokens` und `/v1/messages/count_tokens`. Schalten Sie es nur ein, wenn Ihr Server ihn beantwortet. Sonst beantwortet Kaeso Zählanfragen für seine Modelle mit 400: „The model `…` cannot count tokens.“

Kaeso spricht mit einem eigenen Anbieter das reine Format. Es weiß, wie jeder eingebaute Anbieter von seinem Format abweicht, etwa welche Felder Mistral ablehnt, aber das kann es von Ihrem Server nicht wissen. Ein Feld, das Ihr Server nicht annimmt, kommt als Fehler Ihres Servers zurück. Ein Feld, das das Format nicht tragen kann, lehnt Kaeso ab wie bei jedem Anbieter ([was Kaeso ablehnt](../concepts/format-translation.md#was-kaeso-ablehnt)).

## Was Kaeso nicht wissen kann

models.dev kennt die Modelle Ihres Servers nicht, also kennt Kaeso sie auch nicht. Wenn Sie ein Modell hinzufügen, geben Sie alles an, was es braucht:

- **Preise.** Ein- und Ausgabepreis, und Cache-Preise, wenn der Server zwischengespeicherte Tokens meldet. Kaeso berechnet jede Anfrage zu diesen Preisen und hält Budgets an ihnen fest ([Limits und Budgets](limits-and-budgets.md)). Setzen Sie `0` für einen Server, den Sie nicht pro Token bezahlen.
- **Limits.** Das Kontextfenster und das Ausgabelimit. Kaeso nutzt sie für Budgets und um das Ausgabelimit zu ergänzen, das Messages verlangt.
- **Fähigkeiten.** Eingaben, Ausgaben, Tool-Aufrufe, Reasoning, strukturierte Ausgabe, Temperatur und Aufwandsstufen. Eine Anfrage, die eine Fähigkeit braucht, die das Modell nicht hat, lehnt Kaeso ab, bevor es Ihren Server aufruft.

Diese Werte bleiben, wie Sie sie setzen. Nichts aktualisiert sie später.

## Seine Modelle hinzufügen

Wählen Sie auf der Seite **Modelle** eines Projekts **Modelle hinzufügen**. Eigene Anbieter stehen in der Anbieterliste neben den eingebauten. Wählen Sie Ihren, und der Dialog fragt die Modellliste des Servers ab, `GET /v1/models`, mit dem Schlüssel des Projekts. Er listet die IDs, die der Server nennt und die der Katalog noch nicht hat.

Wählen Sie eine ID oder **Modell-ID eingeben** und geben Sie dann die Werte des Modells ein. Schlägt die Liste fehl, sagt der Dialog warum, und Sie geben die ID ein. Modelle hinzuzufügen erfordert im Projekt das Recht **Modelle verwalten** ([Modelle und Aliasse](models-and-aliases.md)).

Apps rufen das Modell dann bei seinem Namen auf, mit der ID des Anbieters davor:

```bash tab="curl"
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "model": "research-vllm/llama-3.3-70b",
    "messages": [{"role": "user", "content": "Schreibe ein Haiku über Gateways."}]
  }'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)
completion = client.chat.completions.create(
    model="research-vllm/llama-3.3-70b",
    messages=[{"role": "user", "content": "Schreibe ein Haiku über Gateways."}],
)
print(completion.choices[0].message.content)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});
const completion = await client.chat.completions.create({
  model: "research-vllm/llama-3.3-70b",
  messages: [{ role: "user", content: "Schreibe ein Haiku über Gateways." }],
});
console.log(completion.choices[0].message.content);
```

Der Header `kaeso-provider` der Antwort, die Anfrageprotokolle und die Nutzung nennen den Anbieter bei seiner ID. Die Metriken der Installation zählen alle eigenen Anbieter als `custom`.

## Adressen, die Kaeso erreicht

Kaeso prüft die Adresse jeder Verbindung zu einem eigenen Anbieter nach der DNS-Auflösung, nicht nur beim Hinzufügen. Es lehnt private, Loopback-, Link-Local- und Cloud-Metadaten-Adressen ab, etwa `10.0.0.5`, `127.0.0.1` und `169.254.169.254`, außer die Installation erlaubt das Netz. Reines `http` erreicht nur die Netze, die die Installation erlaubt. Kaeso folgt keinen Weiterleitungen eines eigenen Anbieters und ruft ihn ohne Proxy auf.

Einen Anbieter mit einer solchen Adresse hinzuzufügen schlägt fehl mit „`base_url` zeigt auf eine Adresse, die diese Installation nicht erlaubt.“ Wird eine Adresse später nicht mehr erlaubt, schlagen ihre Anfragen mit 502 fehl: „The custom provider `…` is at an address this install does not allow; ask its operator.“ Betreiber erlauben Netze mit `KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS` ([Konfiguration](../self-hosting/configuration.md#eigene-anbieter)).

> [!WARNING]
> Wer eigene Anbieter verwalten darf, entscheidet, wohin Kaeso die Schlüssel des Anbieters und die Prompts Ihrer Apps schickt. Geben Sie das Recht nur Personen, denen Sie beides anvertrauen würden.

## Eigenen Anbieter ändern oder löschen

Das Menü eines Anbieters hat **Bearbeiten** und **Löschen**. Bearbeiten ändert alles außer der ID. Eine neue Basis-URL, Schlüsselart oder ein neues Format setzt die [Verfügbarkeit](models-and-aliases.md#verfügbarkeit) seiner Modelle auf **Nicht geprüft** zurück. Bevor ein Anbieter **Kein Schlüssel** nehmen kann, löschen Sie seine Schlüssel.

Einen Anbieter zu löschen löscht auch seine Anbieter-Schlüssel und seine Modelle im Katalog jedes Projekts, mit ihren Aliassen. Anfragen nach diesen Modellen schlagen ab dann fehl. Der Dialog sagt, wie viele Schlüssel und Modelle in wie vielen Projekten wegfallen, und das Audit-Log behält, was entfernt wurde.

## Wenn Kaeso einen Anbieter mit derselben ID aufnimmt

Eine spätere Version von Kaeso könnte einen eingebauten Anbieter aufnehmen, dessen ID Ihre Organisation schon nutzt. In Ihrer Organisation behält Ihr eigener Anbieter die ID: Seine Schlüssel und Anfragen gehen weiter an Ihren Server, nie an den neuen Anbieter. Die Liste markiert ihn mit **Ersetzt** und dem Namen des eingebauten Anbieters.

Um stattdessen den eingebauten Anbieter zu nutzen, löschen Sie Ihren eigenen Anbieter und fügen dann Schlüssel und Modelle für den eingebauten hinzu.

## Rechte

| Aktion | Recht |
| --- | --- |
| Eigene Anbieter und ihre Basis-URLs sehen | **Eigene Anbieter ansehen** |
| Eigene Anbieter hinzufügen, bearbeiten und löschen | **Eigene Anbieter verwalten**, mit einer kürzlichen Anmeldung |

Inhaber und Admins haben beide, Entwickler und Prüfer können ansehen. Jeder in der Organisation sieht den Namen eines eigenen Anbieters in Anbieterlisten, aber nicht seine Basis-URL. Schaltet die Installation eigene Anbieter aus, behält die Organisation sie, aber Anfragen nach ihren Modellen schlagen fehl mit „Custom providers are turned off on this install, so `…` cannot be called.“

## Nächste Schritte

- [Anbieter-Schlüssel](provider-keys.md): Fügen Sie den Schlüssel hinzu, den Ihr eigener Anbieter nimmt.
- [Modelle und Aliasse](models-and-aliases.md): Fügen Sie seine Modelle mit Preisen und Limits einem Projekt hinzu.
- [Konfiguration](../self-hosting/configuration.md#eigene-anbieter): Erlauben Sie die Netze, die eigene Anbieter erreichen dürfen.
