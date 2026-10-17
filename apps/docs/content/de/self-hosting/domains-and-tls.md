---
title: Domains und TLS
description: Die Domain einer Installation, ihr Zertifikat und der Betrieb hinter einem eigenen Proxy.
order: 4
---

## Eine Domain

Eine Installation antwortet unter einer Domain: Caddy leitet `/v1/…` und `/platform/…` an das Gateway weiter und alle übrigen Pfade an die Konsole. Da Konsole und APIs denselben Origin teilen, bleibt das Sitzungscookie der Konsole ein First-Party-Cookie, und es gibt keinen Cross-Origin-Verkehr einzurichten. Web-Apps auf anderen Origins, die die Modell-API aus dem Browser aufrufen, brauchen [`KAESO_CORS_ALLOWED_ORIGINS`](configuration.md#browser-apps).

`KAESO_DOMAIN` in der `.env` legt die Domain fest, die öffentliche URL `KAESO_PUBLIC_URL` folgt daraus als `https://<domain>`. Einrichtungslinks, E-Mails und die Codebeispiele der Konsole verwenden sie.

## Zertifikate

Caddy holt beim ersten Start ein Zertifikat für `KAESO_DOMAIN` von einer öffentlichen Zertifizierungsstelle und erneuert es selbst. Dafür muss vor dem ersten Start:

- der DNS-Eintrag der Domain auf den Server zeigen;
- der Server auf den Ports 80 und 443 aus dem Internet erreichbar sein.

`docker compose up -d --wait` wartet nicht auf das Zertifikat: Caddy meldet healthy, sobald er Anfragen bedient, und holt das Zertifikat einer neuen Domain im Hintergrund; über HTTPS antwortet die Domain also erst, wenn Caddy es hat.

Die Zertifikate liegen im Volume `caddy_data`. Lässt sich keines ausstellen, zeigt `docker compose logs caddy` die Antwort der Zertifizierungsstelle; DNS-Eintrag oder Firewall korrigieren und dann `docker compose restart caddy` ausführen.

Mit `KAESO_DOMAIN=localhost` nutzt Caddy stattdessen ein Zertifikat seiner eigenen lokalen Zertifizierungsstelle; siehe [Auf dem eigenen Rechner ausprobieren](installation.md#auf-dem-eigenen-rechner-ausprobieren).

## Ein eigener Reverse Proxy

Um Kaeso hinter einem vorhandenen Proxy oder Load Balancer zu betreiben, den Dienst `caddy` abschalten und Gateway und Konsole auf der Loopback-Adresse veröffentlichen, und zwar in der `compose.override.yaml`, damit ein Upgrade die Änderung behält:

```yaml
services:
  caddy:
    profiles: [disabled]
  gateway:
    ports:
      - "127.0.0.1:8080:8080"
  console:
    ports:
      - "127.0.0.1:3000:3000"
```

Kaeso mit `docker compose up -d --wait` starten. Ohne Caddy wartet sonst nichts auf Gateway und Konsole; erst `--wait` sorgt dafür, dass Compose zurückkehrt, wenn beide healthy sind.

Dann weiterleiten:

| Pfad | Ziel |
| --- | --- |
| `/v1/` und `/platform/` | Das Gateway, Port 8080 |
| Alles andere | Die Konsole, Port 3000 |

- Erreichen Personen Kaeso unter einer anderen Adresse als `https://` und `KAESO_DOMAIN`, `KAESO_PUBLIC_URL` in der `.env` darauf setzen, etwa `https://kaeso.example.com`.
- Das Puffern von Antworten für `/v1/` abschalten, damit gestreamte Antworten die Apps sofort erreichen.
- `X-Forwarded-For` weitergeben. Das Gateway glaubt dem Header nur von den Netzen in `KAESO_TRUSTED_PROXIES`, standardmäßig den privaten Bereichen. Hinter einem nicht vertrauenswürdigen Proxy sieht jeder Client wie der Proxy aus: Logs zeigen dessen Adresse, und die Anmeldung und die [Drosselung von API-Schlüsseln](configuration.md#drosselung-von-api-schlüsseln) zählen alle Clients als einen.
- Den Metrik-Port 9100 nie weiterleiten.

### Ein Proxy in Docker

Läuft der eigene Proxy in Docker auf demselben Server, ihn über ein Docker-Netzwerk statt über veröffentlichte Ports mit Gateway und Konsole verbinden. Dafür ein Netzwerk nur für den Proxy und Kaeso verwenden: Compose macht den Namen jedes Dienstes in jedem Netzwerk, dem er beitritt, zu einem DNS-Namen. In einem Netzwerk, das andere Stacks mitnutzen, könnten `gateway` und `console` auf deren Container zeigen oder Kaesos Container auf deren Namen antworten. Das Netzwerk anlegen und den beiden Diensten dort Aliase geben, die nicht kollidieren können:

```bash
docker network create kaeso-proxy
```

```yaml
services:
  caddy:
    profiles: [disabled]
  gateway:
    networks:
      kaeso-proxy:
        aliases: [kaeso-gateway]
  console:
    networks:
      kaeso-proxy:
        aliases: [kaeso-console]

networks:
  kaeso-proxy:
    external: true
```

Den Container des Proxys mit `kaeso-proxy` verbinden und an `kaeso-gateway:8080` und `kaeso-console:3000` weiterleiten. Im Compose-Projekt behalten die Dienste ihre Namen, sodass Befehle wie `docker compose logs gateway` gleich bleiben.

## Lange Streams

Proxys und Load Balancer schließen Verbindungen, über die eine Weile keine Bytes fließen, oft nach 60 oder 100 Sekunden. Ein Modell, das vor der Antwort nachdenkt, kann länger schweigen, und eine App in einem OpenAI-Format, die ein Claude-Modell aufruft, erhält von diesem Reasoning nichts. Solange eine gestreamte Antwort offen ist und `KAESO_STREAMS_KEEPALIVE_INTERVAL` lang (Standard 15 Sekunden) nichts an die App geschrieben wurde, schreibt das Gateway deshalb ein Keepalive, das das SDK der App ignoriert: einen SSE-Kommentar (`: keepalive`) für Chat Completions und Responses, ein `ping`-Ereignis für Anthropic Messages.

- Das Intervall deutlich unter dem kürzesten Leerlauf-Timeout auf dem Weg halten; `0s` schaltet Keepalives ab.
- Das Gateway sendet die Header der Antwort, sobald der Anbieter antwortet, noch vor dem ersten Ereignis.
- Die eigenen Grenzen des Gateways für einen Stream sind `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` und `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT`; siehe [Aufrufe an Anbieter](configuration.md#aufrufe-an-anbieter).

[Ablauf einer Anfrage](../concepts/request-flow.md) beschreibt, was geschieht, wenn ein Stream abbricht oder die App verschwindet.
