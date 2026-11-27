---
title: Domains und TLS
description: Stellen Sie Kaeso mit Zertifikat unter Ihrer Domain bereit oder hinter einem Proxy, den Sie schon betreiben.
order: 5
---

Eine Installation antwortet unter einer Domain. Caddy bezieht das Zertifikat, leitet die APIs und den MCP-Server samt seiner Anmeldung an das Gateway weiter und alle anderen Pfade an die Konsole.

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
| `/v1/`, `/v1beta/`, `/v2/`, `/platform/`, `/mcp`, `/mcp/`, `/oauth/`, `/ingest/` und `/.well-known/oauth-` | Das Gateway, Port 8080 |
| Alles andere | Die Konsole, Port 3000 |

Agenten melden sich über `/oauth/` und `/.well-known/oauth-` an und rufen dann `/mcp` auf ([MCP-Server](../api/mcp.md)); MCP-Clients rufen das Portal eines Projekts unter `/mcp/<Projekt-ID>` auf. Ein Proxy, der diese Pfade an die Konsole leitet, hindert jeden Agenten am Verbinden.

Richten Sie den Proxy für Kaeso ein:

- Schalten Sie die Pufferung von Antworten für `/v1/` und `/v1beta/` ab, damit gestreamte Antworten die Apps sofort erreichen.
- Lassen Sie WebSocket-Upgrades auf `/v1/` durch, damit ElevenLabs-Apps [Echtzeit-Sitzungen](../api/elevenlabs.md#echtzeit-sitzungen) öffnen können.
- Lassen Sie Anfragekörper bis zur höchsten [Größengrenze](configuration.md#anfrage--und-antwortkörper) des Gateways durch, standardmäßig 64 MiB, etwa mit `client_max_body_size` bei nginx. Einen größeren Körper beantwortet das Gateway mit 413 im Format der App. Es liest jeden Körper bis zum Ende, bevor es antwortet, bis zu 1 GiB, auch einen, den es ungelesen ablehnt, sodass ein Proxy, der ihn streamt, etwa Traefik, die 413 oder die Ablehnung weitergibt.
- Geben Sie `X-Forwarded-For` weiter, und setzen Sie `KAESO_TRUSTED_PROXIES` in `.env` auf die genaue Adresse Ihres Proxys. Nur von dort glaubt das Gateway den Header. Standardmäßig vertraut das Compose-Paket Loopback und dem Netzwerk `edge`, `EDGE_SUBNET`. Ein Proxy auf dem Server, der die Ports oben aufruft, erreicht das Gateway von der ersten Adresse von `EDGE_SUBNET` aus, standardmäßig `172.30.83.1`. Hinter einem Proxy, dem es nicht vertraut, sehen Protokolle und die [Drosselung von API-Schlüsseln](configuration.md#drosselung-von-api-schlüsseln) jeden Client als den Proxy.
- Erreichen Personen Kaeso unter einer anderen Adresse als `https://` und `KAESO_DOMAIN`, setzen Sie `KAESO_PUBLIC_URL` in der `.env` auf diese Adresse.
- Leiten Sie den Metrik-Port 9100 nie weiter.

Um Caddy zu behalten und ihn stattdessen einfaches HTTP an Ihren Proxy liefern zu lassen, geben Sie dem Installer einen Port als Domain und die Adresse Ihres Proxys: `sh -s -- --domain :80 --public-url https://kaeso.example.com`. Caddy antwortet dann auf Port 80 ohne Zertifikat, und der Installer schreibt `KAESO_PUBLIC_URL` und einen Absender unter dieser Adresse in `.env`. Ohne `--public-url` ist die Adresse `http://localhost` mit dem Port.

### Proxy in Docker anbinden

Läuft Ihr Proxy in Docker auf demselben Server, verbinden Sie ihn über ein Docker-Netzwerk mit Kaeso statt über veröffentlichte Ports. Legen Sie ein Netzwerk nur für den Proxy und Kaeso an:

```bash
docker network create --subnet 172.30.84.0/24 kaeso-proxy
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

Verbinden Sie den Container Ihres Proxys mit einer festen Adresse wie `172.30.84.2` mit `kaeso-proxy`, setzen Sie `KAESO_TRUSTED_PROXIES=172.30.84.2` in `.env` und leiten Sie an `kaeso-gateway:8080` und `kaeso-console:3000` weiter. Die Aliase sind nötig, weil Compose jeden Dienstnamen in jedem Netzwerk, dem der Dienst beitritt, zu einem DNS-Namen macht. In einem Netzwerk, das andere Stacks mitnutzen, könnte `gateway` sonst den Container eines anderen Stacks erreichen.

## Suchmaschinen

Eine Installation bittet Suchmaschinen und KI-Crawler, draußen zu bleiben, bis Sie sie einlassen: Sie indexieren keine ihrer Seiten, und keine wird zum Training von Modellen oder für die Antworten eines Assistenten angeboten. Kaeso verwahrt die Schlüssel Ihrer Anbieter und die Daten Ihrer Organisationen, daher geht seine Adresse standardmäßig niemanden sonst etwas an.

Setzen Sie `KAESO_SEARCH_ENGINES=true` in `.env` und führen Sie `docker compose up -d --wait` aus, wenn man Ihre Anmeldeseite über eine Suche finden soll. Compose gibt die Variable an das Gateway und die Konsole weiter, die sie beide lesen; das Helm-Chart und die Plattform-Vorlagen setzen sie ebenfalls an einer Stelle ([Konfiguration](configuration.md#suchmaschinen)). Die Seite **Allgemein** der Instanzeinstellungen zeigt, wie sie gesetzt ist.

| Was Crawler bekommen | Aus, der Standard | An |
| --- | --- | --- |
| `/robots.txt` | Jeder Pfad gesperrt | `/sign-in` und die Dateien, die sie darstellen, erlaubt, jeder andere Pfad gesperrt |
| `X-Robots-Tag: noindex, nofollow` | Auf jeder Antwort | Auf jeder Antwort außer dieser Seite und ihren Dateien |
| `<meta name="robots" content="noindex, nofollow">` | In jeder Seite | In jeder Seite außer der Anmeldeseite |

Auch wenn sie an ist, bleiben diese draußen:

- `/sign-in` mit einer Query, weil ihr `next` die Seite nennt, auf der jemand war.
- `/sign-up`. Nur die Platform API sagt, ob Ihre Installation Registrierungen annimmt, und Crawler dürfen sie nicht fragen; sie sähen also ein Formular, das eine Installation auf Einladung nicht anbietet.
- `/`, das abgemeldete Besucher zur Anmeldeseite schickt, und jede Seite der Konsole dahinter.
- Geteilte Links, die für die Menschen gedacht sind, denen sie geschickt wurden.
- Die APIs, der MCP-Server und seine Anmeldung. Das Gateway antwortet auf jede Anfrage mit `noindex`, und unter seiner eigenen Adresse sperrt seine `robots.txt` jeden Pfad.

`robots.txt` nennt keinen Crawler, daher gelten ihre Regeln für jeden Crawler, der sie liest, Suchmaschinen und KI-Crawler gleichermaßen, etwa Googlebot, GPTBot und OAI-SearchBot von OpenAI, ClaudeBot von Anthropic, PerplexityBot und CCBot von Common Crawl. Crawler behalten ihre Kopie höchstens einen Tag, und die Konsole lässt sie eine Stunde gelten.

Das sind Bitten, die sich gut erzogene Crawler zu Herzen nehmen, keine Zugriffskontrolle: Alles außer den Anmeldeseiten verlangt ohnehin eine Anmeldung. Eine Suchmaschine, die `robots.txt` befolgt, ruft eine gesperrte Seite nie ab und liest deshalb auch deren `noindex` nie. Verlinken andere Websites Ihre Installation, kann sie deshalb trotzdem die bloße Adresse aufführen, ohne Titel und ohne Text daraus. Ein Assistent, der einen Link öffnet, weil jemand ihn darum gebeten hat, kann `robots.txt` für diesen einen Besuch übergehen und sieht, was jeder abgemeldete Besucher sieht.

## Lange Streams

Proxys und Load Balancer schließen Verbindungen, über die eine Weile keine Bytes fließen, oft nach 60 oder 100 Sekunden. Ein Modell, das vor der Antwort nachdenkt, kann länger still sein. Solange ein Stream offen ist und `KAESO_STREAMS_KEEPALIVE_INTERVAL` lang (standardmäßig 15 Sekunden) nichts geschrieben wurde, schreibt das Gateway deshalb ein Keepalive, das das SDK der App ignoriert:

- Einen SSE-Kommentar, `: keepalive`, für Chat Completions und Responses.
- Ein `ping`-Event für Anthropic Messages.

Halten Sie das Intervall deutlich unter dem kürzesten Leerlauf-Zeitlimit auf dem Weg. Das Gateway sendet die Header der Antwort, sobald der Anbieter antwortet, noch vor dem ersten Event. Seine eigenen Grenzen für einen Stream sind `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` und `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` ([Aufrufe an Anbieter](configuration.md#aufrufe-an-anbieter)).

Der [Ablauf einer Anfrage](../concepts/request-flow.md) erklärt, was passiert, wenn ein Stream abbricht oder die App verschwindet.

## Nächste Schritte

- [E-Mail](email.md): Verbinden Sie einen SMTP-Server, damit Anmeldelinks und Einladungen ankommen.
- [Zustand und Metriken](monitoring.md): Prüfen Sie den Zustand der Installation und lesen Sie ihre Metriken ab.
