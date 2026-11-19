---
title: Webhooks
description: Senden Sie die Benachrichtigungen der Organisation signiert an Bereitschaftstools, Chat und Skripte.
order: 9
---

Ein Webhook sendet die betrieblichen Benachrichtigungen der Organisation, etwa eine Budgetwarnung oder einen abgelehnten Anbieter-Schlüssel, an eine Adresse Ihrer Wahl, sobald sie entstehen. Bereitschaftstools können daraus einen Vorfall eröffnen, ein Chat-Kanal kann ihn zeigen, und ein Skript kann darauf reagieren. Webhooks bekommen ihre Benachrichtigungen unabhängig davon, was jemand für seine E-Mails wählte, und auch auf einer Installation, die keine E-Mails sendet.

**Webhooks** ist eine Seite der **Organisationseinstellungen**. Sie brauchen `webhooks.view`, um sie zu sehen, und `webhooks.manage`, um sie zu ändern ([Rechte](../concepts/rights.md)). Eine Organisation hat höchstens 10 Webhooks.

## Webhook hinzufügen

1. Öffnen Sie **Organisationseinstellungen** › **Webhooks** und wählen Sie **Webhook hinzufügen**.
2. Geben Sie ihm einen Namen und die Adresse, die die Ereignisse empfängt, etwa `https://hooks.example.com/kaeso`.
3. Wählen Sie das Format. **JSON** sendet Kaesos Ereignis, unten beschrieben, für Skripte und Bereitschaftstools. **Slack** sendet eine Textzeile, `{"text": "…"}`, die eingehende Webhooks von Slack und Chat-Tools, die dasselbe lesen, annehmen.
4. Wählen Sie die Benachrichtigungen, die er empfängt, und ob er die der ganzen Organisation oder eines Projekts bekommt.
5. Wählen Sie **Webhook hinzufügen** und kopieren Sie das Signaturgeheimnis. Es wird nur einmal gezeigt; Ihr Empfänger prüft damit jede Zustellung.

Die Adresse muss eine öffentliche `https`-Adresse sein. Eine Installation kann mit `KAESO_WEBHOOKS_ALLOWED_NETWORKS` weitere Netze erlauben, über `http` oder `https` ([Konfiguration](../self-hosting/configuration.md#webhooks)). Kaeso prüft jede Verbindung nach der DNS-Auflösung, sendet ohne Proxy und folgt keinen Weiterleitungen.

Das Menü eines Webhooks hat **Test senden**, **Ausschalten** oder **Einschalten**, **Geheimnis rotieren** und **Löschen**. **Test senden** sendet sofort ein Ereignis `webhook.test`, nur einmal versucht, und sagt, ob es ankam. **Geheimnis rotieren** ersetzt das Signaturgeheimnis sofort, der Empfänger braucht das neue also vor der nächsten Zustellung. Ein geöffneter Webhook zeigt, was er empfängt, und seine Zustellungen der letzten 7 Tage, die neuesten 50, jede mit ihren Versuchen und der letzten Antwort des Empfängers.

## Was ein Webhook empfangen kann

| Benachrichtigung | Ereignistypen |
| --- | --- |
| Budget-Warnungen | `budget.alert` |
| Limit erreicht | `limit.reached` |
| Ihr Schlüssel läuft ab | `key.expiring` |
| Modell abgekündigt | `model.deprecated` |
| Modell wird bald abgeschaltet | `model.shutdown_soon`, `model.moved` |
| Wechsel von Modelllinien | `line.moved`, `line.move_available` |
| Inhaltsprotokollierung geändert | `project.content_logging_changed` |
| MCP-Server brauchen Aufmerksamkeit | `mcp_server.signed_out`, `mcp_server.tool_held` |
| Sicherheitseinstellungen geändert | `organization.security_changed`, `share.created` |
| Mitgliederänderungen | `organization.member_joined`, `organization.member_left` |
| Anbieter-Schlüssel brauchen Aufmerksamkeit | `provider_key.refused`, `provider_key.signed_out`, `provider_key.spend_limit_reached`, `provider_key.unlinked` |
| Modelländerungen | `model.delisted`, `model.price_changed` |

Das sind die Benachrichtigungen über die Organisation und ihre Projekte ([E-Mails](account.md#e-mails-wählen)). Die eigenen Benachrichtigungen einer Person, etwa zur Sicherheit ihres Kontos oder zu einem Export, den sie startete, gehen nie an einen Webhook. Ein Webhook mit einem Projekt bekommt nur dessen Ereignisse; einer ohne bekommt jedes Ereignis der Organisation und ihrer Projekte. Ablaufende Schlüssel meldet er für jeden Schlüssel, nicht nur dessen Inhaber.

## Das Ereignis

Eine JSON-Zustellung ist ein `POST` mit `content-type: application/json` und einem Inhalt wie diesem:

```json
{
  "id": "msg_01kb2c9w6t3m5q7r8s0v1x2y4z",
  "type": "budget.alert",
  "created_at": "2026-11-16T09:30:00Z",
  "organization": { "id": "org_01kb2c8a4b5c6d7e8f9g0h1j2k", "name": "Acme" },
  "project": { "id": "proj_01kb2c8m3n4p5q6r7s8t9v0w1x", "name": "Checkout" },
  "summary": "Checkout used 80% of its monthly spend budget",
  "data": {
    "limit_id": "lim_01kb2c8y2z3a4b5c6d7e8f9g0h",
    "kind": "spend_budget",
    "period": "month",
    "amount": "100",
    "used": "80.42",
    "percent": 80,
    "applies_to": { "type": "project" },
    "resets_at": "2026-12-01T00:00:00Z"
  },
  "url": "https://kaeso.example.com/orgs/org_01kb2c8a4b5c6d7e8f9g0h1j2k/projects/proj_01kb2c8m3n4p5q6r7s8t9v0w1x/limits"
}
```

- `id` ist die ID des Ereignisses, in jedem Versuch dieselbe.
- `project` ist `null` für ein Ereignis der Organisation.
- `summary` ist die Zeile der Benachrichtigung auf Englisch.
- `url` öffnet die Seite der Konsole, um die es geht.
- IDs haben die Form der [Platform API](../api/platform-api.md). Beträge sind Zeichenketten: US-Dollar für Ausgaben, sonst Token oder Anfragen.

`data` enthält:

| Typ | `data` |
| --- | --- |
| `budget.alert` | `limit_id`, `kind` (`token_budget` oder `spend_budget`), `period`, `amount`, `used`, `percent` (die überschrittene Schwelle), `applies_to` (`{"type": "project"}` oder `member` bzw. `key` mit seinem `name`), `resets_at` (`null` bei einer Gesamtsumme) |
| `limit.reached` | `limit_id`, `kind`, `period`, `amount`, `applies_to`, `resets_at` |
| `key.expiring` | `key_id`, `key` (sein Name), `owner` (eine Benutzer-ID, `null` für den Schlüssel eines Dienstkontos), `expires_at`, `days` |
| `project.content_logging_changed` | `on`, `by` (wer es änderte) |
| `mcp_server.signed_out` | `server_id`, `server` (sein Name) |
| `mcp_server.tool_held` | `server_id`, `server`, `tool`: ein Tool, das sich änderte und auf Freigabe wartet |
| `organization.security_changed` | `change` (`sign_in_rules`, `single_sign_on`, `agent_policy`, `key_policy`, `platform_key_created` oder `platform_key_rotated`), `platform_key` (sein Name, bei den letzten beiden), `by` |
| `share.created` | `title` (der des Links), `audience` (`anyone` oder `members`), `by` ([Nutzung teilen](share-usage.md)) |
| `organization.member_joined` | `person`, `sso` (ob sie über Single Sign-on kam) |
| `organization.member_left` | `person`, `removed_by` (`null`, wenn sie selbst ging) |
| `provider_key.refused` | `provider_key_id`, `provider`, `name`, `default`, `projects`, `refusal` mit `status` und `message` des Anbieters, `billing` (ein Abrechnungsproblem) sowie `first_at` und `last_at` |
| `provider_key.signed_out` | `provider_key_id`, `provider`, `name`, `default`, `projects`, `signed_out_at` |
| `provider_key.spend_limit_reached` | `provider_key_id`, `provider`, `name`, `default`, `projects`, `refused_at`, `resets_at` (wann der Anbieter spätestens wieder Anfragen durchlässt) |
| `provider_key.unlinked` | `provider_key_id`, `provider`, `name`, `default`, `projects`, `unlinked_at` |
| `model.delisted` | `catalogue_model_id`, `provider_id`, `provider`, `model` |
| `model.deprecated` | `catalogue_model_id`, `provider_id`, `provider`, `model`, `shutdown_on` (`null` ohne Datum), `replacement` ([Modell-Lebenszyklus](../concepts/model-lifecycle.md)) |
| `model.shutdown_soon` | `catalogue_model_id`, `provider_id`, `provider`, `model`, `shutdown_on`, `days_left`, `replacement`, `requests_7d` (die Anfragen des Projekts an das Modell in den letzten 7 Tagen) |
| `model.moved` | `catalogue_model_id`, `provider_id`, `provider`, `model`, `replacement`, `aliases` (wie viele umgestellt wurden) |
| `line.moved` | `catalogue_model_id` (der Linie), `provider_id`, `provider`, `line`, `from`, `to`, `prices`: die geänderten Eingabe- und Ausgabepreise, `from` und `to` je Million Token, und `policy`: `follow`, `delay`, `approved` (jemand wählte **Wechseln**) oder `back` ([Modelllinien](models-and-aliases.md#modelllinien)) |
| `line.move_available` | `catalogue_model_id`, `provider_id`, `provider`, `line`, `from`, `to`, `prices`: KPMR hat eine Linie gewechselt, bei der das Projekt zuerst fragt |
| `model.price_changed` | `catalogue_model_id`, `provider_id`, `provider`, `model`, `prices`: jeder geänderte Preis (`input`, `output`, `cache_read`, `cache_write`, `cache_write_1h`) mit seinen alten und neuen US-Dollar je Million Token, `from` und `to`, `null` wo er keinen Preis hatte oder hat |
| `webhook.test` | `webhook_id`, `name` |

`person`, `by` und `removed_by` sind Namen von Personen; `by` und `removed_by` sind `null`, wenn keine Person die Änderung machte, etwa ein Plattform-Schlüssel. Eine Modelländerung kommt einmal für jedes Projekt, dessen Katalog das Modell hat. Spätere Versionen können neue Typen und neue Felder in `data` bringen; ignorieren Sie die, die Sie nicht kennen.

## Zustellung prüfen

Kaeso signiert jede Zustellung, wie [Standard Webhooks](https://www.standardwebhooks.com/) es festlegt, sodass dessen Bibliotheken sie prüfen können. Jede Anfrage hat drei Header:

| Header | Was er enthält |
| --- | --- |
| `webhook-id` | Die `id` des Ereignisses. Ein Empfänger, der eine ID zweimal sieht, hat dasselbe Ereignis erneut |
| `webhook-timestamp` | Wann dieser Versuch gesendet wurde, in Sekunden seit 1970 |
| `webhook-signature` | `v1,` und der Base64-HMAC-SHA256 von `{webhook-id}.{webhook-timestamp}.{body}`, mit den Bytes des Geheimnisses nach `whsec_` als Schlüssel, Base64-dekodiert |

Prüfen Sie die Signatur über den Inhalt genau wie empfangen, bevor Sie ihn lesen, und lehnen Sie einen Zeitstempel ab, der mehr als fünf Minuten von Ihrer Uhr abweicht:

```js title="verify.mjs"
import { createHmac, timingSafeEqual } from "node:crypto";

export function verify(secret, headers, body) {
  const id = headers["webhook-id"];
  const timestamp = headers["webhook-timestamp"];
  if (Math.abs(Date.now() / 1000 - Number(timestamp)) > 300) return false;
  const key = Buffer.from(secret.replace(/^whsec_/, ""), "base64");
  const expected = createHmac("sha256", key).update(`${id}.${timestamp}.${body}`).digest();
  return headers["webhook-signature"].split(" ").some((s) => {
    const [version, signature] = s.split(",");
    const given = Buffer.from(signature ?? "", "base64");
    return version === "v1" && given.length === expected.length && timingSafeEqual(given, expected);
  });
}
```

## Zustellungen und Versuche

- **Wann.** Ein Ereignis geht innerhalb von Sekunden nach seiner Benachrichtigung hinaus.
- **Zugestellt.** Eine Antwort `2xx` innerhalb von 10 Sekunden stellt das Ereignis zu. Jede andere Antwort, auch eine Weiterleitung, oder keine Antwort ist ein gescheiterter Versuch.
- **Versuche.** Ein gescheitertes Ereignis wird nach 1, 5, 15 und 60 Minuten erneut versucht, fünf Versuche insgesamt, jeder neu signiert. Nach dem fünften ist es endgültig gescheitert.
- **Mindestens einmal.** Ereignisse können doppelt und in anderer Reihenfolge kommen: Unterscheiden Sie sie an `webhook-id` und ordnen Sie sie nach `created_at`.
- **Ausgeschaltet.** Ein Webhook, dessen Versuche 20-mal in Folge scheitern, wird ausgeschaltet: Seine wartenden Ereignisse fallen weg, das Audit-Log hält es fest, und wer `webhooks.manage` hat, bekommt eine E-Mail. **Einschalten** sendet ihm wieder neue Ereignisse und beginnt seine Zählung neu.
- **Aufbewahrt.** Zustellungen bleiben 7 Tage.

Das Gateway zählt jeden Versuch in `kaeso_webhook_deliveries_total{outcome}` ([Überwachung](../self-hosting/monitoring.md#metriken)). Webhooks gehören zur [Platform API](../api/platform-api.md) unter `orgs/{org}/webhooks`, und Hinzufügen, Ändern, Rotieren und Löschen steht im [Audit-Log](organizations-and-projects.md#audit-log-lesen).

## Nächste Schritte

- [Limits und Budgets](limits-and-budgets.md): Legen Sie die Budgets fest, deren Warnungen ein Webhook überbringen kann.
- [Konto und Sprache](account.md): Wählen Sie, welche Benachrichtigungen Sie per E-Mail erreichen.
