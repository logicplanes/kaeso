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
| Arbeitsspeicher | 4 GB, davon nutzt ClickHouse etwa 2 GB |
| Speicherplatz | 20 GB, dazu Platz für Anfrageprotokolle |

[Updates mit einem Klick](upgrades.md#updates-mit-einem-klick) brauchen außerdem Docker Engine 25 oder neuer.

## Netzwerk

Die Installation braucht eine Domain, deren DNS-Eintrag auf den Server zeigt, und diese Verbindungen:

| Richtung | Ports oder Hosts | Wofür |
| --- | --- | --- |
| Eingehend | TCP-Ports 80 und 443, dazu UDP-Port 443 für HTTP/3 | Das Zertifikat und HTTPS |
| Ausgehend | `api.openai.com`, `api.anthropic.com` | Die Anbieter, die Sie nutzen |
| Ausgehend | `models.dev` | Modelldaten, einmal am Tag aktualisiert |
| Ausgehend | `api.github.com` | Die Suche nach neuen Kaeso-Releases, zweimal am Tag |
| Ausgehend | `github.com`, `objects.githubusercontent.com`, `ghcr.io` | Updates mit einem Klick |
| Ausgehend | Ihr SMTP-Server | Anmeldelinks, Einladungen, Warnungen und Sicherheitshinweise ([E-Mail](email.md)) |

Ohne Internetzugang arbeitet Kaeso mit den Modelldaten, die im Release enthalten sind, und Sie aktualisieren von Hand. Schalten Sie die ausgehenden Abfragen ab, wie [Installationen ohne Internetzugang](upgrades.md#installationen-ohne-internetzugang) zeigt.

## Speicherplatz für Anfrageprotokolle

Anfrageprotokolle liegen in ClickHouse. Planen Sie je Million Anfragen so viel Speicherplatz ein:

| Protokollierung | Je Million Anfragen |
| --- | --- |
| Nur Metadaten: Modell, Status, Token, Kosten und Zeiten | Etwa 0,1 GB |
| Metadaten und Inhalte, bei etwa 2,5 KB Text je Anfrage | Etwa 0,65 GB |

Die Inhalte wachsen mit der Länge Ihrer Prompts und Antworten. Die Aufbewahrung begrenzt die Gesamtmenge. Bei einer Million Anfragen am Tag belegen die Standardfristen etwa 6 GB Metadaten (90 Tage) und 16 GB Inhalte (30 Tage).

Jede Organisation wählt ihre Aufbewahrung auf ihrer Seite **Protokollierung**, innerhalb der Höchstwerte der Instanz ([Anfrageprotokolle](../console/request-logs.md)). Die Seite **System** der Instanz zeigt, wie viel Platz die Protokolle belegen.
