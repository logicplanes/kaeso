---
title: Modell-Lebenszyklus
description: Wie Kaeso erfährt, dass ein Anbieter ein Modell abschaltet, die Projekte warnt, die es noch aufrufen, und sie auf den Nachfolger umstellt.
order: 5
---

Anbieter schalten Modelle zu festen Terminen ab, und eine Anfrage an ein Modell nach seinem Abschalttag scheitert. Kaeso verfolgt jede Abschaltung von der Ankündigung des Anbieters bis zum letzten Tag: [KPMR](../kpmr/index.md) veröffentlicht die Daten und den Nachfolger, den der Anbieter empfiehlt, und das Gateway ermittelt, welche Projekte das Modell noch aufrufen, warnt sie rechtzeitig und stellt sie mit einem Klick um, oder von selbst, wo ein Projekt es erlaubt.

## Was KPMR veröffentlicht

Jedes Modell im Katalog hat einen `lifecycle`: die Tage, an denen der Anbieter die Abschaltung angekündigt hat, das Modell abkündigt und abschaltet, den empfohlenen Nachfolger `replacement` und die Quelle der Ankündigung `source`, jeweils `null` bis zur Ankündigung ([Katalogformat](../kpmr/catalogue.md#lebenszyklus)). Der `status` des Modells wird an diesen Tagen `deprecated` und dann `retired`, und ein abgeschaltetes Modell bleibt im Katalog. KPMR prüft jedes Datum, bevor es es veröffentlicht ([Abschaltungen und Modelllinien](../kpmr/lifecycle-and-lines.md)).

## Wo ein auslaufendes Modell erscheint

- **Modelle:** Ein Modell mit Abschaltdatum trägt **Abschaltung** und den Tag, und sein Tooltip nennt den Nachfolger. Ein Hinweis erscheint, wenn ein Modell des Projekts innerhalb von 90 Tagen abgeschaltet wird.
- **Tab „Läuft aus“:** Jedes Modell des Projekts, das sein Anbieter abschaltet, die nächste Abschaltung zuerst. Jedes öffnet ein Panel mit den Daten, dem Nachfolger und dem Preisunterschied, den Aliassen und Routenschritten, die das Modell aufrufen (Fallbacks und Zweige von Aufteilungen eingeschlossen), und wer es in den letzten 30 Tagen aufgerufen hat: Schlüssel, Mitglieder, Dienstkonten und Client-Apps anhand des User-Agents, den sie senden. Jeder Aufrufer führt zu seinen Anfragen im [Anfrageprotokoll](../console/request-logs.md), und ein Diagramm zeigt die Anfragen pro Tag seit der Ankündigung, sodass Sie sehen, ob die Umstellung vorankommt.
- **Übersicht der Organisation:** Ein Hinweis listet die Projekte, die auslaufende Modelle nutzen, mit der ersten Abschaltung jedes Projekts.

Aufrufer und Diagramm brauchen `usage.view`, der Rest `models.view`.

## Antworten tragen die Daten

Eine Antwort eines Modells mit angekündigten Daten trägt drei Standard-Header, sodass SDKs, Proxys und Ihre eigene Protokollierung die Warnung sehen, ohne dass jemand die Konsole öffnet:

```http
Deprecation: @1793750400
Sunset: Thu, 04 Feb 2027 00:00:00 GMT
Link: <https://kpmr.kaeso.app/models/anthropic/claude-sonnet-5-5>; rel="successor-version", <https://example.com/deprecations>; rel="deprecation"; type="text/html"
```

`Deprecation` ([RFC 9745](https://www.rfc-editor.org/rfc/rfc9745)) ist der Tag der Abkündigung als Unix-Sekunden, `Sunset` ([RFC 8594](https://www.rfc-editor.org/rfc/rfc8594)) der Abschalttag, und `Link` verweist auf die KPMR-Seite des Nachfolgers und die Ankündigung des Anbieters.

## Nach der Abschaltung

Eine Anfrage, die ein abgeschaltetes Modell nennt, direkt oder über einen Aliasschritt, scheitert mit `410` und einem Kaeso-Fehler, der den Tag und den Nachfolger nennt, statt mit der `404` des Anbieters:

```text
The model `anthropic/claude-sonnet-4-5-20250929` was retired on 2026-11-30; its provider recommends `claude-sonnet-5-5`.
```

In einem Alias scheitert der Schritt als nicht verfügbar, sodass die Route auf ihren nächsten Schritt [zurückfällt](alias-routes.md#fallbacks).

## Auf den Nachfolger umstellen

**Auf Nachfolger umstellen** im Panel eines auslaufenden Modells zeigt jeden Routenschritt, der sich ändern würde, und den Preisunterschied, und ändert sie dann in einem Schritt: Es fügt den Nachfolger mit den Preisen von KPMR zum Katalog hinzu, wenn das Projekt ihn nicht hat, und richtet jeden Aliasschritt, der das Modell aufrief, auf ihn aus. Das Modell selbst bleibt im Katalog, sodass Apps, die es direkt nennen, bis zur Abschaltung weiter funktionieren. Die Umstellung braucht `models.manage` und wird im Audit-Log als `catalogue_model.move` festgehalten.

Zwei Projekteinstellungen unter **Einstellungen → Modellabschaltung** handeln am Abschalttag von selbst. Beide sind standardmäßig aus und brauchen `models.manage`; eine Änderung wird als `project.model_lifecycle` festgehalten:

| Einstellung | Was sie bewirkt |
| --- | --- |
| **Aliasse bei Abschaltung umstellen** | Am Abschalttag stellt Kaeso die Aliasse und Routenschritte, die das Modell aufrufen, auf seinen Nachfolger um, wie **Auf Nachfolger umstellen**, und sendet eine Benachrichtigung und ein Webhook-Ereignis `model.moved` |
| **Abgeschaltete Modelle mit dem Nachfolger beantworten** | Nach der Abschaltung beantwortet der Nachfolger eine Anfrage, die das abgeschaltete Modell nennt, statt dass sie scheitert. `kaeso-model` nennt das Modell, das geantwortet hat, und das Anfrageprotokoll zeigt das abgeschaltete Modell als übersprungenen Schritt. Standardmäßig aus, weil sich Preis und Verhalten ändern |

Der Nachfolger antwortet nur, wenn der Aufrufer ihn nutzen darf. Kaeso fügt ihn am Abschalttag zum Katalog hinzu, wenn das Projekt ihn nicht hat.

## Warnungen

Zwei Benachrichtigungen im Bereich **Projekt** Ihrer [E-Mails](../console/account.md#e-mails-wählen) gehen an die Mitglieder eines Projekts, die seine Modelle sehen, und nur für Modelle im Katalog dieses Projekts:

| Benachrichtigung | Wann |
| --- | --- |
| Modell abgekündigt | KPMR veröffentlicht ein Abschaltdatum oder einen Nachfolger für ein Modell, das das Projekt nutzt |
| Modell wird bald abgeschaltet | 30, 7 und 1 Tag vor der Abschaltung, solange das Projekt das Modell in den letzten 7 Tagen aufgerufen hat, und wenn Kaeso am Abschalttag seine Aliasse umgestellt hat |

Jede kommt auch als [Webhook-Ereignis](../console/webhooks.md): `model.deprecated`, `model.shutdown_soon` und `model.moved`, mit der ID des Katalogmodells, den Daten, dem Nachfolger und, bei `model.shutdown_soon`, den Anfragen der letzten 7 Tage. Das Gateway prüft stündlich.

## In der Platform API und MCP

| Endpunkt | Was er tut | Recht |
| --- | --- | --- |
| `GET /orgs/{org}/projects/{project}/deprecations` | Die auslaufenden Modelle mit ihren Aliassen, Routenschritten und Aufrufern | `models.view` |
| `GET /orgs/{org}/projects/{project}/deprecations/{model}/move` | Was eine Umstellung ändern würde | `models.view` |
| `POST /orgs/{org}/projects/{project}/deprecations/{model}/move` | Stellt auf den Nachfolger um | `models.manage` |
| `GET`, `PUT /orgs/{org}/projects/{project}/model-lifecycle` | Die beiden Einstellungen | `models.view`, `models.manage` |
| `GET /orgs/{org}/deprecations` | Die Projekte der Organisation, die auslaufende Modelle nutzen | `org.view` |

Kaesos [MCP-Server](../api/mcp.md) bietet dieselben Aktionen als Werkzeuge an. Der Zähler `kaeso_retired_model_requests_total{outcome}` zählt Schritte, die ein abgeschaltetes Modell nannten, danach, ob sie abgelehnt oder vom Nachfolger beantwortet wurden ([Monitoring](../self-hosting/monitoring.md)).

## Nächste Schritte

- [Abschaltungen und Modelllinien](../kpmr/lifecycle-and-lines.md): Sehen Sie, woher die Daten kommen und wie KPMR sie prüft.
- [Alias-Routen](alias-routes.md): Leiten Sie Anfragen mit Fallbacks über mehrere Modelle.
- [Webhooks](../console/webhooks.md): Empfangen Sie die Warnungen in Ihren eigenen Systemen.
