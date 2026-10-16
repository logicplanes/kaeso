---
title: Domains und TLS
description: Die Domain, unter der eine Installation antwortet, ihr Zertifikat und der Betrieb hinter einem eigenen Proxy.
order: 4
---

## Eine Domain

Eine Installation antwortet unter einer Domain. Konsole, Modell-API und Platform API teilen sie sich: Caddy leitet `/v1/…` und `/platform/…` an das Gateway und jeden anderen Pfad an die Konsole. Die gemeinsame Domain hält das Sitzungscookie der Konsole first-party, sodass kein Cross-Origin-Verkehr zu konfigurieren ist.

`KAESO_DOMAIN` in `.env` legt die Domain fest. Die öffentliche URL des Gateways, `KAESO_PUBLIC_URL`, ergibt sich daraus als `https://<domain>`. Sie erscheint in Einrichtungslinks, in Einladungs- und Anmelde-E-Mails und in den Codebeispielen der Konsole, und sie muss `https` sein, außer `http://localhost` für die Entwicklung.

## Zertifikate

Caddy holt beim ersten Start ein Zertifikat für `KAESO_DOMAIN` von einer öffentlichen Zertifizierungsstelle und erneuert es selbst. Damit das funktioniert:

- Der DNS-Eintrag der Domain muss vor dem ersten Start auf den Server zeigen.
- Die Ports 80 und 443 müssen aus dem Internet erreichbar sein.

Caddy speichert seine Zertifikate im Volume `caddy_data`. Lässt sich das Zertifikat nicht ausstellen, nennt `docker compose logs caddy` den Grund. Den DNS-Eintrag oder die Firewall korrigieren und Caddy mit `docker compose restart caddy` neu starten.

Mit `KAESO_DOMAIN=localhost` nutzt Caddy stattdessen ein Zertifikat seiner eigenen lokalen Zertifizierungsstelle, das der Browser einmal bestätigen lässt.

## Ein eigener Reverse Proxy

Um Kaeso hinter einen bereits betriebenen Proxy oder Load Balancer zu stellen, den Dienst `caddy` weglassen, etwa in einer eigenen Kopie der Compose-Datei, sonst nichts veröffentlichen und so weiterleiten:

| Pfad | An |
| --- | --- |
| `/v1/` und `/platform/` | das Gateway, Port 8080 |
| alles andere | die Konsole, Port 3000 |

- `KAESO_PUBLIC_URL` in `.env` auf die URL setzen, die Personen nutzen, etwa `https://kaeso.example.com`, wenn sie nicht `https://` und `KAESO_DOMAIN` ist.
- Das Puffern von Antworten für `/v1/` abschalten, damit gestreamte Antworten die Apps erreichen, sobald sie eintreffen.
- `X-Forwarded-For` weitergeben. Das Gateway glaubt ihm nur von Adressen in `KAESO_TRUSTED_PROXIES`, standardmäßig die privaten Netzwerke. Mit einem nicht vertrauenswürdigen Proxy davor sieht jeder Client wie der Proxy aus: Logs zeigen die Adresse des Proxys, und die Drosselung von Anmeldungen und die [Drosselung von API-Schlüsseln](configuration.md#drosselung-von-api-schlüsseln) zählen alle Clients zusammen.
- Den Metrik-Port 9100 nie weiterleiten.

## Lange Streams

Proxys und Load Balancer schließen Verbindungen, über die eine Weile keine Bytes laufen, viele nach 60 oder 100 Sekunden. Ein Modell, das vor der Antwort nachdenkt, kann länger still sein, und eine App in einem OpenAI-Format, die mit einem Claude-Modell spricht, sieht nichts von diesem Nachdenken. Deshalb schreibt das Gateway, solange eine gestreamte Antwort offen ist und `KAESO_STREAMS_KEEPALIVE_INTERVAL` (15 Sekunden) lang nichts an die App geschrieben wurde, ein Keepalive, das das SDK der App ignoriert: einen SSE-Kommentar (`: keepalive`) für Chat Completions und Responses, ein `ping`-Event für Anthropic Messages.

- Das Intervall deutlich unter dem kürzesten Idle-Timeout auf dem Weg halten; `0s` schaltet Keepalives ab.
- Das Gateway sendet die Header der Antwort, sobald der Anbieter antwortet, vor dessen erstem Event.
- Die eigenen Grenzen des Gateways für einen Stream sind `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` und `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT`; siehe [Konfiguration](configuration.md#aufrufe-an-anbieter).

Was geschieht, wenn ein Stream abbricht oder die App verschwindet, beschreibt der [Ablauf einer Anfrage](../concepts/request-flow.md).
