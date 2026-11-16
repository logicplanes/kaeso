---
title: Guardrails
description: Lassen Sie eigene Dienste Anfragen prüfen, bevor ein Modell sie erhält, und Antworten, bevor Apps sie erhalten.
order: 4
---

Eine Guardrail ist ein HTTP-Dienst Ihrer Organisation, den Kaeso fragt, bevor eine Anfrage an ein Modell geht, und auf Wunsch, bevor die Antwort des Modells zurückgeht. Sie kann die Anfrage durchlassen, blockieren oder ihren Text umschreiben. [Eingebaute Regeln](#eingebaute-regeln) erledigen die häufigsten Prüfungen in Kaeso selbst, ohne einen Dienst von Ihnen. Kaeso weiß, welcher Schlüssel, welches Mitglied und welches Projekt jede Anfrage gesendet hat, und das Anfrageprotokoll behält jedes Urteil.

Sie brauchen `guardrails.view`, um **Organisationseinstellungen** › **Guardrails** zu sehen, und `guardrails.manage`, um dort etwas zu ändern.

## Eine Guardrail hinzufügen

1. Öffnen Sie **Organisationseinstellungen** › **Guardrails** und wählen Sie **Guardrail hinzufügen**.
2. Geben Sie einen Namen und die Adresse ein, an die Kaeso sendet, etwa `https://guardrail.example.com/check`. Apps sehen den Namen, wenn die Guardrail sie blockiert.
3. Wählen Sie, was sie prüft: **Anfragen**, **Antworten** oder beides. Wählen Sie für Antworten, ob gestreamte Antworten ungeprüft hinausgehen (**Ungeprüft streamen**) oder auf die Prüfung warten (**Zurückhalten und prüfen**).
4. Legen Sie das Zeitlimit fest: 2000 Millisekunden, wenn Sie es nicht ändern, höchstens 10000.
5. Wählen Sie, was geschieht, wenn die Guardrail nicht antworten kann: **Anfrage ablehnen** oder **Durchlassen**.
6. Wählen Sie **Guardrail hinzufügen** und kopieren Sie das Signatur-Secret. Es wird nur einmal angezeigt.

Eine Adresse in einem privaten Netz funktioniert nur, wenn die Installation ihr Netz in `guardrails.allowed_networks` erlaubt ([Konfiguration](../self-hosting/configuration.md#guardrails)). Einfaches `http` funktioniert nur in diesen Netzen, und Kaeso folgt nie einer Weiterleitung.

Das Menü einer Zeile bietet **Bearbeiten**, **Testen**, **Neues Secret** und **Löschen**. **Neues Secret** ersetzt das Signatur-Secret sofort. **Testen** sendet einen Beispiel-Prompt, oder einen Prompt und eine Antwort, so wie eine Anfrage es täte, und zeigt das Urteil; vom Beispiel wird nichts gespeichert.

## Die Projekte wählen

Eine Guardrail prüft nichts, solange kein Projekt sie nutzt. Öffnen Sie die **Einstellungen** des Projekts und wählen Sie unter **Guardrails** die Option **Guardrails wählen**. Haken Sie die Guardrails an und bringen Sie sie in eine Reihenfolge: Eine Anfrage durchläuft sie von oben nach unten, und jede sieht den Text, den die vorige übrig ließ.

Nur Personen mit `guardrails.manage` in der Organisation wählen sie. Die Admins eines Projekts sehen die Liste, können aber keine Guardrail entfernen: Sie ist die Richtlinie der Organisation.

## Eingebaute Regeln

Eingebaute Regeln finden Secrets und personenbezogene Daten in einer Anfrage, ohne dass Sie einen eigenen Dienst betreiben. Wählen Sie **Guardrail hinzufügen** und dann **Eingebaute Regeln**, geben Sie ihnen einen Namen und wählen Sie, was jeder Detektor tut: **Maskieren** ersetzt jeden Wert durch einen Platzhalter wie `[EMAIL_1]`, **Blockieren** lehnt die Anfrage mit 400 ab und nennt, was gefunden wurde, nie den Wert, und **Nur zählen** lässt sie durch und zählt die Werte.

| Detektor | Findet |
| --- | --- |
| Secrets und API-Schlüssel | Schlüssel der Anbieter, die Kaeso unterstützt, und Kaesos eigene `sk-kso-…`-Schlüssel, AWS-Zugriffsschlüssel-IDs, GitHub- und GitLab-Tokens als `[API_KEY_1]`; PEM-Blöcke privater Schlüssel als `[PRIVATE_KEY_1]`; JSON Web Tokens als `[TOKEN_1]` |
| E-Mail-Adressen | Adressen wie `ada@example.com`, als `[EMAIL_1]` |
| Telefonnummern | Nummern in E.164, etwa `+4930123456`, als `[PHONE_1]` |
| IBANs | IBANs, deren Mod-97-Prüfung stimmt, als `[IBAN_1]` |
| Kartennummern | 13 bis 19 Ziffern, auch mit Leerzeichen oder Bindestrichen, deren Luhn-Prüfung stimmt, als `[CARD_1]` |
| IP-Adressen | IPv4- und IPv6-Adressen, als `[IP_1]` |

Derselbe Wert erhält innerhalb einer Anfrage immer denselben Platzhalter, sodass das Modell zwei Personen weiterhin unterscheiden kann. Die Regeln lesen den Text des Systemprompts und der Nachrichten, Tool-Ergebnisse und die Argumente von Tool-Aufrufen; Medien und Tool-Definitionen bleiben, wie sie sind.

Fügen Sie bis zu 20 eigene Muster hinzu: einen Namen aus Kleinbuchstaben, Ziffern und `_` und einen regulären Ausdruck von höchstens 500 Zeichen in der Syntax von Rusts `regex`-Crate, die in linearer Zeit zur Textlänge sucht. Ein Treffer von `order_id` wird zu `[ORDER_ID_1]`.

- **Im Anfrageprotokoll maskieren** speichert Platzhalter im [Anfrageprotokoll](request-logs.md), auch für Werte, die die Regeln nur zählen. Die Anfrage geht dann unverändert hinaus, während das Protokoll keinen Wert behält.
- **In der Antwort wiederherstellen** setzt die ursprünglichen Werte wieder ein, wo die Antwort des Modells ihre Platzhalter nennt, bevor die Antwort die App erreicht. In einem Stream hält Kaeso höchstens die Länge eines Platzhalters zurück, sodass auch einer gefunden wird, der auf zwei Teile verteilt ist. Streams erhalten ihren Text wiederhergestellt, ganze Antworten auch die Argumente ihrer Tool-Aufrufe.

**Regeln testen** prüft einen Beispieltext, bevor Sie die Regeln speichern; Kaeso speichert davon nichts. Namen, Postanschriften und andere frei formulierte personenbezogene Daten brauchen ein Sprachmodell: Nutzen Sie dafür [einen eigenen Dienst](#eine-guardrail-hinzufügen). Der Inspektor zeigt, wie viele Werte jede Art fand, nie die Werte, und `kaeso_masked_values_total{detector, action}` zählt sie.

## Was Kaeso prüft

Guardrails prüfen Chat-Anfragen: OpenAI Chat Completions und Responses, Anthropic Messages, Gemini und den Playground. Embeddings, Reranking, Entscheidungen, Bilder, Audio, Video, Echtzeit-Sitzungen und Token-Zählungen gehen ungeprüft durch.

Die Anfragephase läuft einmal pro Anfrage: nachdem Kaeso sie gelesen hat und weiß, dass der Aufrufer das Modell nutzen darf, und vor dem [Antwort-Cache](response-cache.md) und dem ersten Schritt einer [Alias-Route](../concepts/alias-routes.md). Eine blockierte Anfrage wird nie aus dem Cache beantwortet, und jeder Schritt einer Route erhält den geprüften Text.

Ein Aufrufer über einer seiner Anfrageraten wird abgelehnt, bevor Kaeso die Guardrail fragt. Eine blockierte Anfrage erreicht keinen Anbieter und kostet nichts, zählt aber als Anfrage im Protokoll und für die Anfrageraten.

Die Antwortphase läuft auf einer ganzen Antwort, bevor Kaeso sie für die App schreibt, auch auf einer Antwort aus dem Antwort-Cache. Eine gestreamte Antwort wird nur geprüft, wenn die Guardrail Streams zurückhält: Kaeso liest dann den ganzen Stream des Anbieters, prüft ihn und streamt ihn an die App, sodass die App länger auf das erste Token wartet.

## Die Anfrage

Kaeso sendet JSON an die Adresse der Guardrail:

```json
{
  "id": "req_01KA2Z8M3C0Y6V1W9T4H5J7Q2E.request",
  "phase": "request",
  "request_id": "req_01KA2Z8M3C0Y6V1W9T4H5J7Q2E",
  "organization": "org_01K9ZB2M7VQ6CG3N8R4T5W1X2Y",
  "project": "proj_01K9ZB3D1F8H2J4K6M8N0P2Q4R",
  "key": "key_01K9ZB4E2G9J3K5M7P9R1T3V5X",
  "member": "usr_01K9ZB1A6C8E0G2J4L6N8Q0S2U",
  "service_account": null,
  "model": "claude-haiku-4-5",
  "format": "anthropic_messages",
  "source": "api",
  "messages": [
    { "role": "system", "content": [{ "type": "text", "text": "Answer in one sentence." }] },
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "What is in this picture?" },
        { "type": "image", "media_type": "image/png", "size": 48213 }
      ]
    }
  ]
}
```

- `phase` ist `request` oder `reply`. Eine Prüfung der Antwort fügt `reply` hinzu, eine `assistant`-Nachricht mit den Teilen der Antwort.
- `model` ist der Name, den die App gesendet hat, auch ein Alias. `format` ist das Format der App, und `source` ist `api`, `playground` oder `test`; ein Test nennt kein Projekt.
- Ein Teil ist `text`, `refusal`, `tool_call` (mit `id`, `name` und `arguments` als JSON-Text), `tool_result` (mit `call_id`, `is_error` und eigenem `content`), `json`, `reasoning` oder `other`. Bilder, Audio, Video und Dokumente sind `image`, `audio`, `video` und `document` mit ihrem `media_type` und ihrer `size` in Bytes, nie mit ihren Bytes.

## Die Antwort

Die Guardrail antwortet mit einem JSON-Objekt und einem 2xx-Status:

```json
{ "action": "allow" }
```

```json
{ "action": "block", "message": "Customer data can't go to a model.", "category": "pii" }
```

```json
{
  "action": "replace",
  "category": "pii",
  "messages": [
    { "role": "system", "content": [{ "type": "text", "text": "Answer in one sentence." }] },
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "What is in this picture?" },
        { "type": "image", "media_type": "image/png", "size": 48213 }
      ]
    }
  ]
}
```

- `category` ist bei jeder Aktion optional und wird im Protokoll gespeichert, höchstens 64 Zeichen.
- `message` ist optional; Kaeso hängt sie an den Fehler der App an, höchstens 300 Zeichen.
- `replace` sendet die Nachrichten zurück, die Kaeso gesendet hat, mit denselben Rollen und Teilen in derselben Reihenfolge. Kaeso übernimmt ihren Text, die Argumente von Tool-Aufrufen und Tool-Ergebnisse und lässt Medien, IDs und Namen, wie sie waren. Für eine Antwort enthält `messages` die eine `assistant`-Nachricht. Ein zurückgehaltener Stream lässt sich nicht umschreiben.

Kaeso ignoriert Felder, die es nicht kennt. Eine Antwort, die keine dieser ist, ein Status außer 2xx, ein `replace` anderer Form oder keine Antwort innerhalb des Zeitlimits gilt als Fehler: Dann entscheidet die Einstellung **Bei einem Fehler** der Guardrail.

## Die Signatur prüfen

Kaeso signiert jede Anfrage wie eine Zustellung nach [Standard Webhooks](https://www.standardwebhooks.com). Die Header `webhook-id`, `webhook-timestamp` und `webhook-signature` tragen die ID der Zustellung, die Zeit in Unix-Sekunden und `v1,` gefolgt vom base64-kodierten HMAC-SHA256 von `id.timestamp.body`. Der Schlüssel ist das Signatur-Secret ohne sein Präfix `whsec_`, aus base64 dekodiert. Jede Bibliothek für Standard Webhooks prüft das; lehnen Sie Anfragen ab, deren Zeitstempel älter als einige Minuten ist.

## Was Apps sehen

Eine blockierte Anfrage erhält 400 im eigenen Format der App, mit der Fehlerart für Inhaltsfilter. Die Meldung nennt die Guardrail und fügt ihren Grund an, etwa „The guardrail `PII check` blocked this request: Customer data can't go to a model.“ Eine blockierte Antwort sagt stattdessen „blocked the model's reply“. Kaeso fällt nicht auf einen anderen Schritt einer Route zurück, da jeder Schritt dieselbe Anfrage erhielte.

Scheitert eine Guardrail, die bei Fehlern ablehnt, erhält die App 503, in den OpenAI-Formaten 500: „The guardrail `PII check` could not check this request, so Kaeso refused it; try again shortly.“

Jede geprüfte Antwort trägt `kaeso-guardrails`, eine Structured-Field-Liste mit einem Eintrag pro Guardrail: ihrem Namen, `phase`, `action`, `category`, `failure` und `dur` in Millisekunden.

```http
kaeso-guardrails: "PII check";phase=request;action=replace;category="pii";dur=18
```

Das [Anfrageprotokoll](request-logs.md) zeigt dasselbe im Inspektor unter **Guardrails**, und der Filter **Guardrail** findet Anfragen, die eine Guardrail durchließ, blockierte oder umschrieb. `kaeso_guardrail_runs_total{phase, action}` und `kaeso_guardrail_duration_seconds{phase}` zählen sie in den [Metriken](../self-hosting/monitoring.md).

## Nächste Schritte

- [Anfrageprotokolle](request-logs.md): Finden Sie die Anfragen, die eine Guardrail blockierte oder umschrieb.
- [Ablauf einer Anfrage](../concepts/request-flow.md): Sehen Sie, wo Guardrails zwischen den Prüfungen des Gateways laufen.
- [Fehler](../api/errors.md): Lesen Sie die Fehler, die Apps erhalten, wenn eine Guardrail eine Anfrage ablehnt.
