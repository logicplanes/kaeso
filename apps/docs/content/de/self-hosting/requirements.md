---
title: Anforderungen
description: Was ein Server braucht, um Kaeso zu betreiben.
order: 1
---

## Server

- Linux mit Docker Engine und Docker Compose 2.24 oder neuer. Die Images gibt es für `linux/amd64` und `linux/arm64`. [Updates mit einem Klick](upgrades.md#updates-mit-einem-klick) brauchen Docker Engine 25 oder neuer, dessen API-Version 1.44 der Updater spricht.
- Mindestens 2 vCPUs und 4 GB Arbeitsspeicher, wovon ClickHouse etwa 2 GB belegt.
- Mindestens 20 GB Speicherplatz, dazu Platz für Anfrageprotokolle.

## Netzwerk

- Eine Domain, deren DNS-Eintrag auf den Server zeigt.
- Die Ports 80 und 443 aus dem Internet erreichbar, für das Zertifikat und für HTTPS. Wo UDP-Port 443 offen ist, antwortet Caddy auch über HTTP/3.
- Ausgehendes HTTPS zu den genutzten Anbietern (`api.openai.com`, `api.anthropic.com`) und zu `models.dev`, von wo das Gateway einmal am Tag die Modelldaten aktualisiert. Ohne Internetzugang zu `models.dev` arbeitet eine Installation mit den Modelldaten, die das Release mitbringt; die Aktualisierung lässt sich mit `KAESO_MODELS_DEV_SYNC=false` in der [`compose.override.yaml`](installation.md#lokale-änderungen) abschalten.
- Ausgehendes HTTPS zu `api.github.com`, wo das Gateway zweimal täglich nach neuen Kaeso-Releases sucht, und für [Updates mit einem Klick](upgrades.md#updates-mit-einem-klick) zu `github.com`, `objects.githubusercontent.com` und `ghcr.io`. Eine Installation ohne es aktualisiert von Hand; die Prüfung wie unter [Upgrades](upgrades.md#installationen-ohne-internetzugang) beschrieben abschalten.
- Ein SMTP-Server für Anmeldelinks, Einladungen, Budgetwarnungen und Sicherheitshinweise. Siehe [E-Mail](email.md).

## Festplatte für Anfrageprotokolle

Anfrageprotokolle liegen in ClickHouse. Gemessen an 200.000 synthetischen Anfragen mit je etwa 2,5 KB Anfrage- und Antworttext, nachdem ClickHouse seine Parts zusammengeführt hatte:

| Daten | Belegt pro Million Anfragen |
| --- | --- |
| Metadaten: Modell, Status, Token, Kosten, Zeiten | Etwa 63 MB |
| Inhalt: Anfragen und Antworten | Etwa 545 MB, also 0,21 Byte pro Byte Text |
| Nutzungsaggregate | Höchstens 40 MB, meist deutlich weniger |

Als Richtwert etwa 0,1 GB pro Million Anfragen ohne Inhaltsprotokollierung und 0,65 GB mit ihr einplanen und den Inhaltsanteil an die Größe der eigenen Prompts und Antworten anpassen. Die Aufbewahrung begrenzt die Gesamtmenge: Bei einer Million Anfragen pro Tag und der Standardaufbewahrung von 90 Tagen für Metadaten und 30 Tagen für Inhalte sind das etwa 6 GB Metadaten und 16 GB Inhalte.

Jede Organisation legt ihre Aufbewahrung unter [Protokollierung](../console/request-logs.md#aufbewahrung) fest, innerhalb der Höchstwerte der Instanz. Die Seite **Protokollierung** der Organisation und die Seite **System** der Instanz zeigen die tatsächlichen Größen.
