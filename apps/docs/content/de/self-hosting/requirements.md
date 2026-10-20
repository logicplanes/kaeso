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

[Der Updater](upgrades.md#der-updater) braucht außerdem Docker Engine 25 oder neuer.

## Netzwerk

Die Installation braucht eine Domain, deren DNS-Eintrag auf den Server zeigt, und diese Verbindungen:

| Richtung | Ports oder Hosts | Wofür |
| --- | --- | --- |
| Eingehend | TCP-Ports 80 und 443, dazu UDP-Port 443 für HTTP/3 | Das Zertifikat und HTTPS |
| Ausgehend | `api.openai.com`, `api.anthropic.com`, `api.deepseek.com`, `api.x.ai`, `api.together.ai`, `api.groq.com`, `openrouter.ai` oder die Basis-URL, die Sie für einen Anbieter festlegen ([Anbietereinstellungen](configuration.md#anbietereinstellungen)) | Die Anbieter, die Sie nutzen |
| Ausgehend | `models.dev` | Modelldaten, einmal am Tag aktualisiert |
| Ausgehend | `update.kaeso.app` | Die Suche nach neuen Kaeso-Releases, zweimal am Tag |
| Ausgehend | `acme-v02.api.letsencrypt.org`, `acme.zerossl.com` | Caddy, um das Zertifikat zu holen und zu erneuern |
| Ausgehend | `ghcr.io`, `pkg-containers.githubusercontent.com` | Docker, um die Images von Kaeso zu laden, wenn Sie installieren oder aktualisieren |
| Ausgehend | `registry-1.docker.io`, `auth.docker.io`, `production.cloudflare.docker.com` | Docker, um die Images von Caddy, PostgreSQL, ClickHouse, Valkey und dem Socket-Proxy des Updaters zu laden |
| Ausgehend | `github.com`, `release-assets.githubusercontent.com` | Der Download von `compose.yaml` und `release.json` eines Releases, von Hand oder durch den Updater |
| Ausgehend | `api.github.com` | Der Updater und `gh attestation verify`, um die Attestierungen eines Releases zu holen |
| Ausgehend | Ihr SMTP-Server | Anmeldelinks, Einladungen, Warnungen und Sicherheitshinweise ([E-Mail](email.md)) |
| Ausgehend | Die Identitätsanbieter Ihrer Organisationen | Single Sign-on, wenn eine Organisation einen verbindet ([Anmeldung](sign-in.md)) |

Ohne Internetzugang arbeitet Kaeso mit den Modelldaten, die im Release enthalten sind, und Sie aktualisieren von Hand. Schalten Sie die ausgehenden Abfragen ab, wie [Installationen ohne Internetzugang](upgrades.md#installationen-ohne-internetzugang) zeigt.

## Speicherplatz für Anfrageprotokolle

Anfrageprotokolle liegen in ClickHouse. Planen Sie je Million Anfragen so viel Speicherplatz ein:

| Protokollierung | Je Million Anfragen |
| --- | --- |
| Nur Metadaten: Modell, Status, Token, Kosten und Zeiten | Etwa 0,1 GB |
| Metadaten und Inhalte, bei etwa 2,5 KB Text je Anfrage | Etwa 0,65 GB |

Die Inhalte wachsen mit der Länge Ihrer Prompts und Antworten. Die Aufbewahrung begrenzt die Gesamtmenge. Bei einer Million Anfragen am Tag belegen die Standardfristen etwa 6 GB Metadaten (90 Tage) und 16 GB Inhalte (30 Tage).

Jede Organisation wählt ihre Aufbewahrung auf ihrer Seite **Protokollierung**, innerhalb der Höchstwerte der Instanz ([Anfrageprotokolle](../console/request-logs.md)). Die Seite **System** der Instanz zeigt, wie viel Platz die Protokolle belegen.
