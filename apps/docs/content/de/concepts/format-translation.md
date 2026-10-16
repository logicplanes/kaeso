---
title: Formatübersetzung
description: Wie KPP Anfragen zwischen den Formaten von OpenAI und Anthropic überträgt und was es ablehnt.
order: 2
---

KPP, das Kaeso Provider Protocol, ist ein kanonisches Format für Modellanfragen, mit einem Codec für jedes API-Format, der dorthin und zurück übersetzt. Das Gateway nutzt es für jede Anfrage, sodass eine App in jedem dieser Formate ein Modell jedes der beiden Anbieter aufrufen kann.

| Format | Endpunkte | Streaming |
| --- | --- | --- |
| Anthropic Messages | `/v1/messages`, `/v1/messages/count_tokens` | ja |
| OpenAI Chat Completions | `/v1/chat/completions` | ja |
| OpenAI Responses | `/v1/responses`, `/v1/responses/input_tokens` | ja |
| OpenAI Embeddings | `/v1/embeddings` | nein |

`GET /v1/models` und `GET /v1/models/{id}` listen die Modelle des Projekts in der Form von Anthropic, wenn die Anfrage `anthropic-version` trägt, sonst in der Form von OpenAI. Embeddings gehen nur an Embedding-Modelle von OpenAI.

## Drei Regeln

1. **Gleiches Format, kein Verlust.** Eine Anfrage, die im selben Format an einen Anbieter geht, in dem sie kam, kommt so an, wie sie gesendet wurde. Nur der Modellname wird zur Modell-ID des Anbieters, und ein Stream fordert immer die Nutzung an, die die App nur sieht, wenn sie danach gefragt hat.
2. **Ablehnen, nie weglassen.** Wechselt eine Anfrage das Format, wird ein Feld, das das Ziel nicht umsetzen kann, mit einem Fehler abgelehnt, der es nennt. Nur Hinweise, die nicht ändern, was das Modell schreibt, werden weggelassen.
3. **Signiertes Reasoning geht nur zurück nach Hause.** Verschlüsseltes oder signiertes Reasoning wird immer nur an das Format und den Anbieter zurückgesendet, die es ausgegeben haben.

## Was übertragen wird

- Unterhaltungen: Systemanweisungen, Züge von Benutzer und Assistent, Text, Bilder und Dokumente. Eine Systemnachricht mitten in einer Anthropic-Unterhaltung erreicht OpenAI als spätere Systemnachricht.
- Tools: Funktions-Tools, Tool-Auswahl, parallele Tool-Aufrufe, Tool-Aufrufe und Tool-Ergebnisse.
- Tool-Namen und Aufruf-IDs, die das Ziel nicht annimmt, werden auf feste Weise umgeschrieben und in der Antwort zurückübersetzt: andere Zeichen werden zu `_`, der Name wird passend gekürzt, und ein Hash des Originals wird angehängt. OpenAI nimmt Namen aus bis zu 64 Buchstaben, Ziffern, `_` und `-` an; Anthropic bis zu 128.
- Strukturierte Ausgabe: JSON-Schemas.
- Sampling: Temperatur, Top P, maximale Token und Stoppsequenzen, wo das Ziel sie hat.
- Die ID des Endbenutzers: Anthropic `metadata.user_id`, OpenAI `safety_identifier`.
- Reasoning: Aufwandsstufen und Thinking-Budgets werden ineinander umgerechnet.
- Streams: Event für Event in das Format der App übersetzt.

## Was abgelehnt wird

| Ziel | Abgelehnt, mit einem Fehler |
| --- | --- |
| Anthropic | `seed`, Frequency- und Presence-Penalties, eine Temperatur über 1, das Ausgabeformat `json_object`, ein JSON-Schema mit Beschreibung oder mit `strict: false`, Systemnachrichten mitten in einer Unterhaltung aus einem anderen Format, Verweigerungen im Verlauf, Bilder außer JPEG, PNG, GIF und WebP, Dokumente außer PDF und reinem Text, die Aufwandsstufe `minimal` |
| OpenAI Chat Completions | `top_k`, mehr als vier Stoppsequenzen, Textdokumente, Dokument-URLs, Dokumenttitel, Kontext und Zitate, Bild-Datei-IDs, Bilder und Dokumente außerhalb von Benutzernachrichten |
| OpenAI Responses | `top_k`, `seed`, Penalties, Stoppsequenzen, Textdokumente, Dokumenttitel, Kontext und Zitate |
| Jeder Anbieter | Mehr als eine Auswahl (`n`); gespeicherter Zustand: `store: true`, `previous_response_id`, `conversation`, `background` |
| Ein anderes Format | Felder des App-Formats, für die KPP keinen Platz hat, etwa `logprobs`, `logit_bias` und `service_tier` von Chat Completions; integrierte Tools eines anderen Formats, etwa die Websuche, und ihre Ergebnisse; Zitate; Datei-IDs eines anderen Anbieters |

Hinweise, die nicht ändern, was das Modell schreibt, werden weggelassen, wenn das Ziel keinen Platz dafür hat: Cache-Hinweise, Beta- und Versions-Header, Bilddetail, Dateinamen, Prompt-Cache-Schlüssel, `thinking.display` von Anthropic und das Fehler-Flag eines Tool-Ergebnisses, das OpenAI nicht hat; der Text des Ergebnisses sagt weiterhin, was fehlschlug. Anfragen, die in Responses übersetzt werden, tragen `store: false`, sodass OpenAI nichts speichert.

## Reasoning

| Von der App | Ziel | Wird zu |
| --- | --- | --- |
| Ein Thinking-Budget | OpenAI | Einer Aufwandsstufe: unter 4.096 Token `low`, 4.096 bis 16.383 `medium`, ab 16.384 `high` |
| Abgeschaltetes Thinking | OpenAI | Der Aufwandsstufe `none` |
| Eine Aufwandsstufe von `low` bis `max` | Claude mit adaptivem Denken | Adaptivem Denken mit dieser Aufwandsstufe |
| Eine Aufwandsstufe von `low` bis `max` | Claude ohne adaptives Denken | Einem Budget: `low` 2.048, `medium` 8.192, `high`, `xhigh` und `max` 24.576 Token |
| Die Aufwandsstufe `none` | Claude | Keinem Thinking |

- **Aufwand und Budget.** Eine Aufwandsstufe in derselben Anfrage hat Vorrang vor einem Budget. Ein Claude-Modell gilt als ohne adaptives Denken, wenn sein Katalogeintrag das sagt; Claude 4.5 und früher haben keines. Ein Budget wird an die Regeln von Anthropic angepasst: mindestens 1.024 Token und weniger als `max_tokens`. Ist `max_tokens` 1.024 oder kleiner, beträgt das Budget 1.024, und `max_tokens` wird um 1.024 erhöht, damit die Antwort ihren Platz behält.
- **Thinking und Sampling.** Schaltet Kaeso das Thinking von Claude für eine Anfrage aus einem anderen Format ein, lässt es `temperature`, `top_p` und `top_k` weg, die Anthropic zusammen mit Thinking ablehnt. Ein erzwungener Tool-Aufruf (`required` oder ein benanntes Tool) geht ohne Budget-Thinking, das Anthropic mit einem erzwungenen Tool ablehnt; adaptives Denken behält das erzwungene Tool. Die eigenen Einstellungen einer Anthropic-App werden so gesendet, wie sie ankamen.
- **Tool-Züge von anderswo.** Ist Budget-Thinking eingeschaltet und enthalten die Tool-Aufrufe des letzten Assistenten-Zugs kein Thinking, das Claude wiedergeben kann, etwa weil ein anderer Anbieter sie erzeugt hat, geht diese Anfrage ohne Thinking. Adaptives Denken bleibt eingeschaltet.

## Reasoning zwischen Zügen

Reasoning von einem anderen Anbieter kommt im eigenen Format der App zurück, mit dem Original darin verpackt, damit die App es zurücksenden kann. Eine Responses-App erhält ein Reasoning-Item, das das Original in `encrypted_content` enthält; eine Anthropic-App erhält einen `thinking`-Block, der es in `signature` enthält. Sendet die App es zurück, stellt KPP das Original für den Anbieter wieder her, der es geschrieben hat. Übersetzte Anfragen mit einer anderen Aufwandsstufe als `none` fordern von OpenAI sein verschlüsseltes Reasoning an, damit auch dieses zurückkommen kann.

Chat Completions hat kein Feld für Reasoning. Wird eine Chat-Completions-App von Claude bedient oder von einem OpenAI-Modell über Responses, behält das Gateway das Reasoning vor jedem Tool-Aufruf und fügt es wieder ein, wenn die App das Tool-Ergebnis sendet. Das Modell behält so seinen Gedankengang durch Tool-Schleifen.

- Es liegt für `KAESO_CONTINUITY_REASONING_TTL` (1 Stunde) in Valkey, verschlüsselt mit dem Hauptschlüssel und nur mit dem API-Schlüssel lesbar, der die Anfrage gestellt hat. Es wird nie protokolliert. `0s` schaltet es ab ([Konfiguration](../self-hosting/configuration.md)).
- Reasoning nach dem letzten Tool-Aufruf wird nicht behalten, ebenso wenig Reasoning aus einem fehlgeschlagenen Stream.
- Ist das Reasoning weg (abgelaufen, anderer Schlüssel, Valkey nicht erreichbar), läuft die Anfrage ohne es weiter. `kaeso_reasoning_continuity_total` zählt behaltenes, wiederhergestelltes, fehlendes und nicht verfügbares Reasoning ([Monitoring](../self-hosting/monitoring.md)).

Lesbares Reasoning folgt dem, was die App in ihrem eigenen Format anfordert:

- Eine Responses-App, die `reasoning.summary` setzt, erhält das Thinking von Claude als Reasoning-Zusammenfassungen, gestreamt als `response.reasoning_summary_text.delta`-Events. Das Gateway fordert von Claude zusammengefasstes Thinking an, das Claude Opus 4.7 und später sonst verbergen.
- Eine Anthropic-App mit eingeschaltetem Thinking erhält die Reasoning-Zusammenfassungen eines OpenAI-Modells als Thinking-Text. Das Gateway fordert sie bei OpenAI an, es sei denn, die App hat `thinking.display` auf `omitted` gesetzt. OpenAI kann Zusammenfassungen für Organisationen verweigern, die es nicht verifiziert hat; sein Fehler wird weitergegeben.
- Eine Chat-Completions-App sieht kein Reasoning.

Der Katalog sagt, ob ein Modell Reasoning nutzt (models.dev `reasoning`). Bei einem Modell mit Reasoning schaltet `reasoning: {summary: "auto"}` ohne Aufwandsstufe das Thinking von Claude in seiner Standardtiefe ein: adaptives Denken oder das Budget von 8.192 Token. Das Thinking einer Anthropic-App ohne Aufwandsstufe bringt das verschlüsselte Reasoning von OpenAI für den nächsten Zug zurück. Reasoning wird nie für ein Modell eingeschaltet, das kein Reasoning nutzt oder von dem das nicht bekannt ist, und eine Anfrage, die bei einem solchen Modell eine Aufwandsstufe, Thinking oder ein Budget verlangt, wird abgelehnt.
