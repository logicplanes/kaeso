---
title: Domains und TLS
description: Stellen Sie Kaeso mit Zertifikat unter Ihrer Domain bereit oder hinter einem Proxy, den Sie schon betreiben.
order: 5
---

Eine Installation antwortet unter einer Domain. Caddy bezieht das Zertifikat, leitet `/v1` und `/platform` an das Gateway weiter und alle anderen Pfade an die Konsole.

## Domain festlegen

Legen Sie die Domain vor dem ersten Start in der `.env` fest:

```bash title=".env"
KAESO_DOMAIN=kaeso.example.com
```

Die öffentliche URL ist `https://` und die Domain. Einrichtungslinks, E-Mails und die Codebeispiele der Konsole verwenden sie.

Weil Konsole und APIs denselben Origin teilen, gibt es keinen Cross-Origin-Verkehr einzurichten. Nur Web-Apps auf anderen Origins, die die Modell-API aus dem Browser aufrufen, brauchen [`KAESO_CORS_ALLOWED_ORIGINS`](configuration.md#browser-apps).

## Zertifikat beziehen

Caddy bezieht beim ersten Start ein Zertifikat von einer öffentlichen Zertifizierungsstelle und erneuert es selbst. Bevor Sie Kaeso starten:

1. Richten Sie den DNS-Eintrag der Domain auf den Server.
2. Öffnen Sie die Ports 80 und 443 zum Internet.

HTTPS-Anfragen können eine Minute lang scheitern, während Caddy das Zertifikat bezieht. Führen Sie `docker compose logs caddy` aus, um zu sehen, wann er es hat oder warum nicht. Nachdem Sie den DNS-Eintrag oder die Firewall korrigiert haben, führen Sie `docker compose restart caddy` aus.

Caddy bewahrt seine Zertifikate im Volume `caddy_data` auf. Mit `KAESO_DOMAIN=localhost` signiert er stattdessen eines mit seiner eigenen lokalen Zertifizierungsstelle ([Auf dem eigenen Rechner ausprobieren](installation.md#auf-dem-eigenen-rechner-ausprobieren)).

## Eigenen Reverse Proxy verwenden

Um Kaeso hinter einem Proxy oder Load Balancer zu betreiben, den Sie schon haben, schalten Sie Caddy ab und veröffentlichen Gateway und Konsole auf der Loopback-Adresse:

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

Schreiben Sie das in die `compose.override.yaml`, damit ein Upgrade es beibehält, und starten Sie Kaeso mit `docker compose up -d --wait`. Ohne Caddy wartet sonst nichts auf das Gateway, deshalb sorgt erst `--wait` dafür, dass Compose zurückkehrt, wenn es gesund ist.

Leiten Sie dann diese Pfade weiter:

| Pfad | An |
| --- | --- |
| `/v1/` und `/platform/` | Das Gateway, Port 8080 |
| Alles andere | Die Konsole, Port 3000 |

Richten Sie den Proxy für Kaeso ein:

- Schalten Sie die Pufferung von Antworten für `/v1/` ab, damit gestreamte Antworten die Apps sofort erreichen.
- Geben Sie `X-Forwarded-For` weiter. Das Gateway glaubt dem Header nur von `KAESO_TRUSTED_PROXIES`, standardmäßig den privaten Netzen. Hinter einem Proxy, dem es nicht vertraut, sehen Logs und die [Drosselung von API-Schlüsseln](configuration.md#drosselung-von-api-schlüsseln) jeden Client als den Proxy.
- Erreichen Personen Kaeso unter einer anderen Adresse als `https://` und `KAESO_DOMAIN`, setzen Sie `KAESO_PUBLIC_URL` in der `.env` auf diese Adresse.
- Leiten Sie den Metrik-Port 9100 nie weiter.

### Proxy in Docker anbinden

Läuft Ihr Proxy in Docker auf demselben Server, verbinden Sie ihn über ein Docker-Netzwerk mit Kaeso statt über veröffentlichte Ports. Legen Sie ein Netzwerk nur für den Proxy und Kaeso an:

```bash
docker network create kaeso-proxy
```

Verbinden Sie dann Gateway und Konsole damit, mit Aliasen, die kein anderer Stack verwendet:

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

Verbinden Sie den Container Ihres Proxys mit `kaeso-proxy` und leiten Sie an `kaeso-gateway:8080` und `kaeso-console:3000` weiter. Die Aliase sind nötig, weil Compose jeden Dienstnamen in jedem Netzwerk, dem der Dienst beitritt, zu einem DNS-Namen macht. In einem Netzwerk, das andere Stacks mitnutzen, könnte `gateway` sonst den Container eines anderen Stacks erreichen.

## Lange Streams

Proxys und Load Balancer schließen Verbindungen, über die eine Weile keine Bytes fließen, oft nach 60 oder 100 Sekunden. Ein Modell, das vor der Antwort nachdenkt, kann länger still sein. Solange ein Stream offen ist und `KAESO_STREAMS_KEEPALIVE_INTERVAL` lang (standardmäßig 15 Sekunden) nichts geschrieben wurde, schreibt das Gateway deshalb ein Keepalive, das das SDK der App ignoriert:

- Einen SSE-Kommentar, `: keepalive`, für Chat Completions und Responses.
- Ein `ping`-Event für Anthropic Messages.

Halten Sie das Intervall deutlich unter dem kürzesten Leerlauf-Timeout auf dem Weg. Das Gateway sendet die Header der Antwort, sobald der Anbieter antwortet, noch vor dem ersten Event. Seine eigenen Grenzen für einen Stream sind `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` und `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` ([Aufrufe an Anbieter](configuration.md#aufrufe-an-anbieter)).

Der [Ablauf einer Anfrage](../concepts/request-flow.md) erklärt, was passiert, wenn ein Stream abbricht oder die App verschwindet.

## Nächste Schritte

- [E-Mail](email.md): Verbinden Sie einen SMTP-Server, damit Anmeldelinks und Einladungen ankommen.
- [Zustand und Metriken](monitoring.md): Prüfen Sie den Zustand der Installation und lesen Sie ihre Metriken ab.
