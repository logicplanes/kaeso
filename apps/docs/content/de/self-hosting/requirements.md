---
title: Anforderungen
description: Server, Netzwerkzugang und Speicherplatz, die eine Kaeso-Installation braucht.
order: 1
---

Kaeso läuft auf einem Linux-Server mit Docker. Bemessen Sie den Speicherplatz danach, wie viele Anfragen Sie protokollieren.

## Server

| Ressource | Mindestens |
| --- | --- |
| System | Linux mit Docker Engine und Docker Compose 2.24 oder neuer, auf `linux/amd64` oder `linux/arm64` |
| CPU | 2 vCPUs |
| Arbeitsspeicher | 4 GB. Am meisten nutzt ClickHouse, bis zu seiner Grenze `CLICKHOUSE_MEMORY` ([Container-Grenzen](configuration.md#container-grenzen)) |
| Speicherplatz | 20 GB, dazu Platz für Anfrageprotokolle |

[Der Updater](upgrades.md#der-updater) braucht außerdem Docker Engine 25 oder neuer.

## Arbeitsspeicher und Images

ClickHouse, das die Anfrageprotokolle speichert, braucht mehr Arbeitsspeicher als alle anderen Datenbanken zusammen. Gemessen am 31.10.2026 mit Docker 29.5.2, vor und nach einem Lauf des [Gateway-Benchmarks](../development/benchmark.md):

| Dienst | Arbeitsspeicher im Leerlauf | Arbeitsspeicher nach dem Benchmark | Image |
| --- | --- | --- | --- |
| ClickHouse | 647 MiB | 1,81 GiB | 981 MB |
| PostgreSQL | 75 MiB | 75 MiB | 672 MB |
| Valkey | 15 MiB | 15 MiB | — |
| Gateway | — | — | 117 bis 141 MB |

Mit [ausgeschalteten Anfrageprotokollen](installation.md#festlegen-wie-anfrageprotokolle-gespeichert-werden) betreibt die Installation kein ClickHouse. Das spart im Leerlauf etwa 0,65 GB Arbeitsspeicher, unter Last 1,8 GB, und das Image mit 981 MB. Nutzung, Limits und Budgets funktionieren in beiden Fällen gleich.

## Netzwerk

Die Installation braucht eine Domain, deren DNS-Eintrag auf den Server zeigt, und diese Verbindungen:

| Richtung | Ports oder Hosts | Wofür |
| --- | --- | --- |
| Eingehend | TCP-Ports 80 und 443, dazu UDP-Port 443 für HTTP/3 | Das Zertifikat und HTTPS |
| Ausgehend | Der Host jedes Anbieters, den Sie nutzen ([Unterstützte Anbieter](../concepts/supported-providers.md)), oder die Basis-URL, die Sie für ihn festlegen ([Anbietereinstellungen](configuration.md#anbietereinstellungen)) | Die Anbieter, die Sie nutzen |
| Ausgehend | `models.dev` | Modelldaten, einmal am Tag aktualisiert |
| Ausgehend | `update.kaeso.app` | Die Suche nach neuen Kaeso-Releases, alle 5 Minuten |
| Ausgehend | `acme-v02.api.letsencrypt.org`, `acme.zerossl.com` | Caddy, um das Zertifikat zu holen und zu erneuern |
| Ausgehend | `ghcr.io`, `pkg-containers.githubusercontent.com` | Docker, um die Images von Kaeso zu laden, wenn Sie installieren oder aktualisieren |
| Ausgehend | `registry-1.docker.io`, `auth.docker.io`, `production.cloudflare.docker.com` | Docker, um die Images von PostgreSQL, ClickHouse, Valkey und dem Socket-Proxy des Updaters zu laden |
| Ausgehend | `github.com`, `release-assets.githubusercontent.com` | Der Download von `compose.yaml` und `release.json` eines Releases, von Hand oder durch den Updater |
| Ausgehend | `api.github.com` | Der Updater und `gh attestation verify`, um die Attestierungen eines Releases zu holen |
| Ausgehend | Ihr SMTP-Server | Anmeldelinks, Einladungen, Warnungen und Sicherheitshinweise ([E-Mail](email.md)) |
| Ausgehend | Die Identitätsanbieter Ihrer Organisationen | Single Sign-on, wenn eine Organisation einen verbindet ([Anmeldung](sign-in.md)) |
| Ausgehend | Die Basis-URLs der eigenen Anbieter Ihrer Organisationen | Ihre Anfragen, direkt und nie über einen Proxy ([Eigene Anbieter](configuration.md#eigene-anbieter)) |

Ohne Internetzugang arbeitet Kaeso mit den Modelldaten, die im Release enthalten sind, und Sie aktualisieren von Hand. Schalten Sie die ausgehenden Abfragen ab, wie [Installationen ohne Internetzugang](upgrades.md#installationen-ohne-internetzugang) zeigt.

## Speicherplatz für Anfrageprotokolle

Anfrageprotokolle liegen in ClickHouse und belegen keinen Speicherplatz, wenn sie ausgeschaltet sind. Planen Sie je Million Anfragen so viel Speicherplatz ein:

| Protokollierung | Je Million Anfragen |
| --- | --- |
| Nur Metadaten: Modell, Status, Token, Kosten und Zeiten | Etwa 0,1 GB |
| Metadaten und Inhalte, bei etwa 2,5 KB Text je Anfrage | Etwa 0,65 GB |

Die Inhalte wachsen mit der Länge Ihrer Prompts und Antworten. Die Aufbewahrung begrenzt die Gesamtmenge. Bei einer Million Anfragen am Tag belegen die Standardfristen etwa 9 GB Metadaten (90 Tage) und 16 GB Inhalte (30 Tage).

Jede Organisation wählt ihre Aufbewahrung auf ihrer Seite **Protokollierung**, innerhalb der Höchstwerte der Instanz ([Anfrageprotokolle](../console/request-logs.md)). Die Seite **System** der Instanz zeigt, wie viel Platz die Protokolle belegen.
