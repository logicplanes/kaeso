---
title: Eigene Anbieter
description: Fügen Sie einen eigenen Server, etwa ein selbst betriebenes vLLM oder einen internen Proxy, als Anbieter Ihrer Organisation hinzu.
order: 4
---

Ein eigener Anbieter ist ein Server, den Ihre Organisation selbst hinzufügt, etwa ein selbst betriebenes vLLM, Ollama, LM Studio, TGI oder SGLang oder ein interner Proxy. Kaeso ruft ihn in einem API-Format auf, das es schon übersetzt. Apps erreichen seine Modelle wie die jedes [unterstützten Anbieters](../concepts/supported-providers.md), aus jedem Format und nach denselben Regeln.

## Eigenen Anbieter hinzufügen

1. Öffnen Sie unter **Anbieter** den Reiter **Eigene Anbieter** und wählen Sie **Eigenen Anbieter hinzufügen**.
2. Wählen Sie, was auf dem Server läuft, etwa **vLLM**, **Ollama** oder **LM Studio** ([Server-Software](#server-software)), oder **Anderer Server oder Proxy**, und wählen Sie **Weiter**.
3. Geben Sie die **Basis-URL** des Servers ein, etwa `https://vllm.example.com/v1`, und seinen **API-Schlüssel**. Lassen Sie den Schlüssel leer für einen Server, der keinen nimmt.
4. Wählen Sie **Einstellungen erkennen**. Kaeso fragt den Server, wie es ihn aufruft ([was Kaeso erkennt](#was-kaeso-erkennt)).
5. Prüfen Sie, was Kaeso gefunden hat: den **Name**, die **ID**, die **Basis-URL**, den **Server**, das **API-Format**, ob er **Liefert auch Embeddings** oder **Zählt Tokens** und wie er seinen **Schlüssel** nimmt. Ändern Sie, was nicht stimmt.
6. Wählen Sie **Eigenen Anbieter hinzufügen**. Kaeso prüft die geänderten Einstellungen mit dem Server, fügt dann den Anbieter hinzu und speichert den Schlüssel als seinen Anbieter-Schlüssel.

Ohne das Recht **Anbieter-Schlüssel hinzufügen** nutzt Kaeso den Schlüssel nur, um die Einstellungen zu erkennen; jemand mit dem Recht fügt ihn unter **Anbieter-Schlüssel** hinzu ([Anbieter-Schlüssel](provider-keys.md)). Zuletzt fügen Sie die Modelle des Anbieters einem Projekt hinzu ([seine Modelle hinzufügen](#seine-modelle-hinzufügen)).

Die **ID** beginnt mit dem Host des Servers, bei einer Adresse wie `localhost`, `10.0.0.5` oder `host.docker.internal` mit dem Namen seiner Software, und folgt dem Namen, bis Sie sie ändern. Modellnamen beginnen mit ihr, etwa `research-vllm/llama-3.3-70b`, und sie ändert sich nie. Eine ID besteht aus Kleinbuchstaben und Ziffern mit einzelnen Bindestrichen dazwischen, höchstens 40 Zeichen. Sie darf nicht die ID eines eingebauten Anbieters sein und auch nicht die eines Anbieters in [KPMR](../concepts/kpmr.md), denn Kaeso könnte diesen Anbieter später aufnehmen. Jede Organisation hat ihre eigenen IDs, zwei Organisationen können also beide ein `research-vllm` haben.

Kaeso hängt den Pfad des Formats an die Basis-URL an, etwa `/v1/chat/completions`, wie bei jedem Anbieter. Deshalb entfernt es ein abschließendes `/v1` oder einen eingefügten Endpunkt, etwa `/v1/chat/completions`, und ergänzt `https://` bei einer Adresse ohne Schema. Ein Pfad vor `/v1` bleibt: `https://proxy.example.com/llm/v1` erhält Anfragen unter `https://proxy.example.com/llm/v1/chat/completions`. Ein Server, dessen API nicht unter `/v1` liegt, lässt sich noch nicht hinzufügen.

## Was Kaeso erkennt

Kaeso findet die Einstellungen mit Anfragen, die nichts kosten. Es fragt die Modellliste des Servers ab, `GET /v1/models`, mit dem Schlüssel als `Authorization: Bearer` und als `x-api-key`. Dann sendet es einen leeren Body an den Pfad jedes Formats, den ein Server ablehnt, bevor er etwas erzeugt oder berechnet.

| Einstellung | Wie Kaeso sie findet |
| --- | --- |
| Schlüssel | Die Art, die die Modellliste annimmt; eine Liste in der Form von Anthropic bevorzugt `x-api-key`. Ohne Schlüssel **Kein Schlüssel**. |
| API-Format | Die Form der Liste und der Pfad, der antwortet: `/v1/chat/completions`, `/v1/responses` oder `/v1/messages`. Ein Server mit Chat Completions erhält es, auch wenn er auch Responses hat. |
| Liefert auch Embeddings | Ob `/v1/embeddings` antwortet. |
| Zählt Tokens | Ob `/v1/responses/input_tokens` oder `/v1/messages/count_tokens` antwortet, für Responses und Messages. |
| Server | Nur bei **Anderer Server oder Proxy**: der Besitzer, den die Modellliste nennt, etwa `vllm` oder `sglang`, der `server`-Header, etwa der von llama.cpp, oder ein kostenloses GET an einen Endpunkt der Software selbst: `/api/version` bei Ollama, `/info` bei Text Generation Inference und `/version` bei LocalAI. Ohne Treffer **Anderer Server oder Proxy**. |
| Modelle | Die IDs, die die Liste nennt, mit dem Kontextfenster und dem Ausgabelimit, die sie angibt. |

Einen Server ohne Modellliste findet Kaeso über den Pfad, der antwortet. Erreicht Kaeso den Server nicht, lehnt er den Schlüssel ab oder antwortet er in keinem Format, das Kaeso spricht, sagt der Dialog das neben dem Feld, das Sie korrigieren, und nichts wird hinzugefügt.

## Format wählen

Kaeso wählt das Format, das es erkennt. Wenn Sie es ändern, wählen Sie das Format, in dem Ihr Server antwortet:

| Ihr Server | API-Format |
| --- | --- |
| vLLM, Ollama, LM Studio, TGI, SGLang und die meisten OpenAI-kompatiblen Server | OpenAI Chat Completions |
| Ein Server oder Proxy mit OpenAIs `POST /v1/responses` | OpenAI Responses |
| Ein Server oder Proxy mit Anthropics `POST /v1/messages` | Anthropic Messages |

- **Liefert auch Embeddings:** an, wenn der Server `POST /v1/embeddings` im Format von OpenAI beantwortet. Seine Embedding-Modelle bedienen dann `/v1/embeddings`.
- **Zählt Tokens:** Nur Responses und Messages haben einen Endpunkt für Token-Zählungen, `/v1/responses/input_tokens` und `/v1/messages/count_tokens`. Lassen Sie es nur an, wenn Ihr Server ihn beantwortet. Sonst beantwortet Kaeso Zählanfragen für seine Modelle mit 400: „The model `…` cannot count tokens.“

Bei **Anderer Server oder Proxy** spricht Kaeso das reine Format. Es weiß, wie jeder eingebaute Anbieter von seinem Format abweicht, etwa welche Felder Mistral ablehnt, aber das kann es von Ihrem Server nicht wissen. Ein Feld, das Ihr Server nicht annimmt, kommt als Fehler Ihres Servers zurück. Bei Software, die Kaeso kennt, passt es Anfragen an wie bei einem eingebauten Anbieter ([Server-Software](#server-software)). Ein Feld, das das Format nicht tragen kann, lehnt Kaeso ab wie bei jedem Anbieter ([was Kaeso ablehnt](../concepts/format-translation.md#was-kaeso-ablehnt)).

## Server-Software

Kaeso weiß, wie die Inferenz-Server unten von dem Format abweichen, das sie sprechen. Ein eigener Anbieter, der seinen **Server** nennt, erhält diese Anpassungen bei jeder Anfrage, und die Konsole zeigt das Zeichen der Software überall, wo der Anbieter erscheint:

- `developer`-Nachrichten kommen als `system`-Nachrichten an, denn die Chat-Vorlage eines Modells kennt oft nur `system`. Die Vorlagen von Qwen verwerfen eine `developer`-Nachricht kommentarlos.
- Das Ausgabelimit einer App geht als `max_tokens`, das jeder von ihnen liest; Ollama ignoriert `max_completion_tokens`.
- Die ID des Endnutzers bleibt draußen, ebenso ein Prompt-Cache-Schlüssel von Codex, den diese Server nicht nutzen.
- Das Reasoning eines Modells, in dem Feld, in das die Software es schreibt, erreicht Apps in jedem Format: Claude Code sieht es als Thinking-Blöcke, und Kaeso schickt es in späteren Runden im selben Feld zurück.
- Ein Prompt, der länger ist als der Kontext des Modells, erreicht Apps als zu langer Prompt, sodass Claude Code das Gespräch komprimiert und eine [Alias-Route](../concepts/alias-routes.md) auf ein Modell mit größerem Kontext ausweichen kann. Kaeso kennt die Meldung von vLLM, SGLang, llama.cpp, LMDeploy und den Servern, die auf ihnen aufbauen.

| Server | Erkannt an | Lauscht auf | Hinweise |
| --- | --- | --- | --- |
| Docker Model Runner | Besitzer `docker` | `12434`, unter `/engines` | Geben Sie die Basis-URL mit `/engines` an, etwa `http://host.docker.internal:12434/engines`. |
| Jan | Besitzer `llama.cpp`, `mlx` oder `remote` | `1337` | Schalten Sie den lokalen API-Server in den Einstellungen von Jan ein; für jede Adresse außer `127.0.0.1` braucht er einen Schlüssel. |
| KoboldCpp | Besitzer `koboldcpp` | `5001` | `--password` setzt einen Schlüssel. |
| llama.cpp | Header `server: llama.cpp`, Besitzer `llamacpp` | `9931` seit v0.7.0, `8080` in llamafile | `--host 0.0.0.0` für andere Rechner, `--api-key` für einen Schlüssel. Reasoning ist standardmäßig an. |
| LM Studio | Besitzer `organization_owner` | `1234` | Schalten Sie **Serve on Local Network** für andere Rechner ein und **Require Authentication** für einen Schlüssel. |
| LMDeploy | Besitzer `lmdeploy` | `23333` | `--api-keys` setzt Schlüssel, `--reasoning-parser` und `--tool-call-parser` schalten Reasoning und Tools ein. |
| LocalAI | `/version` | `8080` | `LOCALAI_API_KEY` setzt einen Schlüssel. |
| MLX LM | nicht erkennbar | `8080` | `mlx_lm.server` bietet nur Chat Completions. |
| NVIDIA NIM | nicht erkennbar | `8000` | Wählen Sie es von Hand. |
| Ollama | `/api/version` | `11434` | `OLLAMA_HOST=0.0.0.0` für andere Rechner. Ollama nimmt keinen Schlüssel, wählen Sie also **Kein Schlüssel**. Kaeso lehnt `logit_bias` ab, das Ollama ignoriert. |
| SGLang | Besitzer `sglang` | `30000` | `--api-key` setzt einen Schlüssel, `--reasoning-parser` und `--tool-call-parser` schalten Reasoning und Tools ein. |
| TabbyAPI | Besitzer `tabbyAPI` | `5000` | Braucht einen Schlüssel aus `api_tokens.yml`, außer `disable_auth` ist gesetzt. |
| Text Generation Inference | `/info` | `3000`, `80` in seinem Image | Nur Chat Completions, ohne Reasoning. Kaeso lehnt `logit_bias` ab, das es ignoriert. Hugging Face entwickelt es nicht mehr weiter. |
| vLLM | Besitzer `vllm` | `8000` | `--api-key` setzt einen Schlüssel, `--reasoning-parser` schaltet Reasoning ein, `--enable-auto-tool-choice --tool-call-parser <parser>` schaltet Tools ein. Nimmt Videos als `video_url`. |
| Xinference | Besitzer `xinference` | `9997` | Verlangt einen Schlüssel, außer `XINFERENCE_AUTH_ADVANCED=false`. |

Ein Server, der sich nicht erkennen lässt, oder einer hinter einem Proxy erhält seine Software, wenn Sie sie im ersten Schritt oder unter **Bearbeiten** wählen. Andere Software ändert, wie Kaeso mit dem Server spricht, nie, wohin es Anfragen sendet.

### Einen Server auf dem Rechner des Gateways erreichen

Das Gateway läuft in einem Container, in dem `localhost` der Container selbst ist. Ein Server auf demselben Rechner ist unter `host.docker.internal` erreichbar, etwa `http://host.docker.internal:11434` für Ollama, und muss auf mehr als der Loopback-Adresse lauschen. Docker Desktop kennt diesen Namen; unter Linux fügen Sie ihn dem Gateway in `compose.override.yaml` hinzu:

```yaml title="compose.override.yaml"
services:
  gateway:
    extra_hosts:
      - host.docker.internal:host-gateway
    environment:
      KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS: 172.17.0.1/32
```

Die Adresse gehört zu einem privaten Netz, also muss die Installation sie erlauben ([Adressen, die Kaeso erreicht](#adressen-die-kaeso-erreicht)). Dieser Befehl gibt die Adresse aus, die `host.docker.internal` für das Gateway hat:

```bash
docker run --rm --add-host host.docker.internal:host-gateway alpine getent hosts host.docker.internal
```

Tragen Sie diese Adresse mit `/32` in `KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS` ein und übernehmen Sie die Änderung mit `docker compose up -d --wait`.

## Was Kaeso nicht wissen kann

KPMR kennt Ihren Server nicht, und eine Modellliste nennt nur wenige Werte eines Modells. Wenn Sie ein Modell hinzufügen, trägt Kaeso ein, was es findet, und Sie geben den Rest an:

- **Preise.** Ein- und Ausgabepreis, und Cache-Preise, wenn der Server zwischengespeicherte Tokens meldet. Kaeso trägt sie nie ein, denn ein Preis in KPMR ist der eines anderen Anbieters. Es berechnet jede Anfrage zu diesen Preisen und hält Budgets an ihnen fest ([Limits und Budgets](limits-and-budgets.md)). Setzen Sie `0` für einen Server, den Sie nicht pro Token bezahlen.
- **Limits.** Das Kontextfenster und das Ausgabelimit, aus der Modellliste oder sonst vom gleichnamigen Modell in KPMR. Kaeso nutzt sie für Budgets und um das Ausgabelimit zu ergänzen, das Messages verlangt.
- **Fähigkeiten.** Eingaben, Ausgaben, Tool-Aufrufe, Reasoning, strukturierte Ausgabe, Temperatur und Aufwandsstufen, vom gleichnamigen Modell in KPMR. Eine Anfrage, die eine Fähigkeit braucht, die das Modell nicht hat, lehnt Kaeso ab, bevor es Ihren Server aufruft.

Diese Werte bleiben, wie Sie sie setzen. Nichts aktualisiert sie später.

## Seine Modelle hinzufügen

Wählen Sie auf der Seite **Modelle** eines Projekts **Modelle hinzufügen**. Eigene Anbieter stehen in der Anbieterliste neben den eingebauten. Wählen Sie Ihren, und der Dialog fragt die Modellliste des Servers ab, `GET /v1/models`, mit dem Schlüssel des Projekts. Er listet die IDs, die der Server nennt und die der Katalog noch nicht hat.

Wählen Sie eine ID oder **Modell-ID eingeben** und prüfen Sie dann die Werte des Modells. Kaeso trägt die Limits ein, die die Liste angibt, und die Werte des Modells, das KPMR unter demselben Namen führt, ohne Groß- und Kleinschreibung und einen Besitzer vor dem `/`: `meta-llama/Llama-3.3-70B-Instruct` übernimmt die von `meta-llama/llama-3.3-70b-instruct`. Die Preise geben Sie immer selbst ein. Schlägt die Liste fehl, sagt der Dialog warum, und Sie geben die ID ein. Modelle hinzuzufügen erfordert im Projekt das Recht **Modelle verwalten** ([Modelle und Aliasse](models-and-aliases.md)).

Apps rufen das Modell dann bei seinem Namen auf, mit der ID des Anbieters davor:

```bash tab="curl"
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "model": "research-vllm/llama-3.3-70b",
    "messages": [{"role": "user", "content": "Write a haiku about gateways."}]
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
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
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
  messages: [{ role: "user", content: "Write a haiku about gateways." }],
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
