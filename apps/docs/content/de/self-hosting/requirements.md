---
title: Anforderungen
description: Was ein Server braucht, um Kaeso zu betreiben.
order: 1
---

## Server

- Linux mit Docker Engine und Docker Compose 2.24 oder neuer.
- Mindestens 2 vCPUs und 4 GB Arbeitsspeicher. ClickHouse belegt davon etwa 2 GB.
- Mindestens 20 GB Festplatte, plus Platz für Anfrageprotokolle.

Die Images werden für `linux/amd64` und `linux/arm64` veröffentlicht.

## Netzwerk

- Eine Domain, deren DNS-Eintrag auf den Server zeigt.
- Die Ports 80 und 443, aus dem Internet erreichbar, für das Zertifikat und für HTTPS.
- Ausgehendes HTTPS zu den genutzten Anbietern (`api.openai.com`, `api.anthropic.com`) und zu `models.dev`, wo das Gateway einmal am Tag die Modelldaten aktualisiert. Eine Installation ohne Internetzugang zu `models.dev` funktioniert mit den Modelldaten, die im Release enthalten sind; die Aktualisierung mit `KAESO_MODELS_DEV_SYNC=false` abschalten.
- Ein SMTP-Server für Anmeldelinks, Einladungen, Budget-Warnungen und Sicherheitshinweise. Siehe [E-Mail](email.md).

## Festplatte für Anfrageprotokolle

Anfrageprotokolle liegen in ClickHouse. Gemessen mit dem Schema des Gateways an synthetischem Verkehr, 200.000 Anfragen mit je etwa 2,5 KB Anfrage- und Antworttext, nachdem ClickHouse seine Parts zusammengeführt hatte:

| Daten | Auf der Festplatte pro Million Anfragen |
| --- | --- |
| Metadaten: Modell, Status, Tokens, Kosten, Zeiten | etwa 63 MB |
| Inhalt: Anfragen und Antworten, bei 2,5 KB Text pro Anfrage | etwa 545 MB, etwa 0,21 Byte pro Byte Text |
| Nutzungsaggregate | höchstens 40 MB, meist weit weniger |

Ohne Inhaltsprotokollierung etwa 0,1 GB pro Million Anfragen einplanen und mit ihr 0,65 GB, und den Inhaltsanteil mit der Größe der eigenen Prompts und Antworten skalieren. Die Aufbewahrung begrenzt die Gesamtmenge: Bei einer Million Anfragen am Tag und der Standardaufbewahrung von 90 Tagen für Metadaten und 30 Tagen für Inhalte sind das etwa 6 GB Metadaten und 16 GB Inhalte.

Die Aufbewahrung wird pro Organisation unter [Protokollierung](../console/request-logs.md#aufbewahrung) festgelegt, innerhalb der Höchstwerte der Instanz. Die Einstellungen **Protokollierung** der Organisation und die Seite **System** der Instanz zeigen die tatsächlichen Zahlen.
