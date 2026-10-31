---
title: Konfiguration
description: Jede Einstellung einer Kaeso-Installation, wohin sie gehört und wie das Gateway sie liest.
order: 4
---

Eine Installation hält ihre Einstellungen neben der `compose.yaml`. Die Variablen aus [der ersten Tabelle](#einstellungen-in-der-env) gehören in die `.env`, jede andere Einstellung des Gateways in die `compose.override.yaml`:

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_STREAMS_KEEPALIVE_INTERVAL: 10s
```

Übernehmen Sie eine Änderung mit `docker compose up -d --wait`. Eine Einstellung des Gateways, die nur in der `.env` steht, erreicht das Gateway nie, denn Compose gibt nur die Variablen der ersten Tabelle weiter.

## Einstellungen in der .env

`compose.yaml` liest diese Variablen aus der `.env` und leitet daraus die Datenbank-URLs, die Datei der Hauptschlüssel und die öffentliche URL des Gateways ab.

| Variable | Erforderlich | Legt fest |
| --- | --- | --- |
| `KAESO_DOMAIN` | ja | Die Domain, die Caddy bereitstellt und für die er ein Zertifikat bezieht. Die öffentliche URL ist `https://` und die Domain. |
| `POSTGRES_PASSWORD` | ja | Das Passwort des PostgreSQL-Benutzers `kaeso`, als der sich das Gateway anmeldet. |
| `POSTGRES_SUPERUSER_PASSWORD` | ja | Das Passwort des PostgreSQL-Superusers `postgres`. Kaeso verwendet es nie. |
| `CLICKHOUSE_PASSWORD` | ja | Das Passwort des ClickHouse-Benutzers `kaeso`. Compose prüft es auch, wenn das mitgelieferte ClickHouse aus ist. |
| `KAESO_CLICKHOUSE_URL` | nein | Die URL Ihres eigenen ClickHouse, oder leer, um Anfrageprotokolle auszuschalten ([festlegen, wie Anfrageprotokolle gespeichert werden](installation.md#festlegen-wie-anfrageprotokolle-gespeichert-werden)). |
| `VALKEY_PASSWORD` | ja | Das Passwort, das Valkey von jeder Verbindung verlangt. |
| `KAESO_PUBLIC_URL` | nein | Die öffentliche URL, wenn sie nicht `https://` und die Domain ist, etwa hinter Ihrem eigenen Proxy. |
| `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | `KAESO_SMTP_HOST`, sofern E-Mails nicht nur protokolliert werden | Wie das Gateway E-Mails sendet ([E-Mail](#e-mail)). |
| `KAESO_MAIL_TRANSPORT` | nein | `log`, um E-Mails nur zu protokollieren statt sie zu senden. |
| `KAESO_UPDATER_TOKEN`, `COMPOSE_PROFILES` | nein | Das Token, das Gateway und Updater teilen, und `updater`, um den Updater zu starten ([Der Updater](upgrades.md#der-updater)). |
| `KAESO_DIR` | mit dem Updater | Der Ordner der Installation, der einzige, aus dem der Docker-Proxy des Updaters Bind-Mounts erlaubt. |
| `DOCKER_SOCKET` | nein | Der Pfad des Docker-Sockets für den Updater, wenn er nicht `/var/run/docker.sock` ist. |
| `KAESO_TRUSTED_PROXIES` | nein | Die Netze von Proxys, deren `X-Forwarded-For` das Gateway vertraut, standardmäßig `127.0.0.0/8,::1/128` und `EDGE_SUBNET`. Setzen Sie es auf die Adresse Ihres eigenen Proxys ([eigener Reverse Proxy](domains-and-tls.md#eigenen-reverse-proxy-verwenden)). |
| `EDGE_SUBNET` | nein | Das Subnetz des Netzwerks `edge`, standardmäßig `172.30.83.0/24`, dessen `X-Forwarded-For` das Gateway glaubt. Ändern Sie es, wenn der Server diesen Bereich schon nutzt. |
| `CADDY_MEMORY`, `GATEWAY_MEMORY`, … und `CADDY_PIDS`, `GATEWAY_PIDS`, …, `VALKEY_MAXMEMORY` | nein | Die Speicher- und Prozessgrenzen der Dienste und der Speicher, in dem Valkey seine Daten hält ([Container-Grenzen](#container-grenzen)). |

Verwenden Sie in den Passwörtern nur Buchstaben und Ziffern, wie `openssl rand -hex` sie schreibt, denn sie stehen in Verbindungs-URLs. PostgreSQL behält die Passwörter, die es beim Anlegen seines Volumes hatte; ändern Sie `POSTGRES_PASSWORD` später, sperrt das das Gateway aus. ClickHouse und Valkey übernehmen ihr Passwort bei jedem Start aus der `.env`.

## Container-Grenzen

Jeder Dienst in `compose.yaml` hat eine Speicher- und eine Prozessgrenze. Die Standardwerte passen zum [kleinsten Server](requirements.md#server); erhöhen Sie einen Wert in der `.env`, wenn ein Dienst mehr braucht:

| Dienst | Speicher | Prozesse |
| --- | --- | --- |
| `caddy` | `CADDY_MEMORY`, `512m` | `CADDY_PIDS`, `256` |
| `gateway` | `GATEWAY_MEMORY`, `2g` | `GATEWAY_PIDS`, `1024` |
| `console` | `CONSOLE_MEMORY`, `256m` | `CONSOLE_PIDS`, `128` |
| `postgres` | `POSTGRES_MEMORY`, `2g` | `POSTGRES_PIDS`, `512` |
| `clickhouse` | `CLICKHOUSE_MEMORY`, `4g` | `CLICKHOUSE_PIDS`, `4096` |
| `valkey` | `VALKEY_MEMORY`, `1g` | `VALKEY_PIDS`, `128` |
| `updater` | `UPDATER_MEMORY`, `256m` | `UPDATER_PIDS`, `128` |
| `docker-proxy` | `DOCKER_PROXY_MEMORY`, `128m` | `DOCKER_PROXY_PIDS`, `64` |

Speicher nimmt die Einheiten von Docker, etwa `768m` oder `3g`. Docker beendet einen Dienst, der seine Speichergrenze erreicht, und startet ihn neu. ClickHouse richtet seinen Speicherverbrauch selbst an der Grenze aus.

Valkey hält seine Daten innerhalb von `VALKEY_MAXMEMORY`, standardmäßig `512mb`, was unter `VALKEY_MEMORY` bleibt, damit Valkey nie wegen Speicher beendet wird. Es verdrängt nie einen Schlüssel, um Platz zu schaffen, sodass kein Raten- oder Budgetzähler unbemerkt verschwindet.

Ist Valkey voll, lehnt es neue Schlüssel ab. Bis wieder Platz ist, liest das Gateway aus PostgreSQL statt aus seinem Cache. Es lässt Anfragen unter Ratenlimits und weichen Budgets durch und antwortet unter harten Budgets mit `503`. Den meisten Speicher von Valkey belegt die [Kontinuität des Reasonings](#kontinuität-des-reasonings), die `continuity.reasoning_ttl` begrenzt.

Docker rotiert das Log jedes Dienstes bei 10 MB und behält fünf Dateien.

## Wie das Gateway Einstellungen liest

- **Umgebungsvariablen.** Jede Einstellung hat eine, etwa `KAESO_LOG_LEVEL`. Eine leere Variable gilt als nicht gesetzt.
- **Eine TOML-Datei.** `KAESO_CONFIG=/etc/kaeso/kaeso.toml` liest Einstellungen aus einer Datei, mit den TOML-Schlüsseln unten. Eine Umgebungsvariable hat Vorrang vor der Datei, und ein Schlüssel, der keine Einstellung ist, ist ein Fehler. [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml) setzt jede Einstellung außer den Secrets.
- **Secrets aus Dateien.** Jedes Secret kann stattdessen aus der Datei kommen, die `<VARIABLE>_FILE` nennt, etwa `KAESO_MASTER_KEYS_FILE=/run/secrets/master_keys`. Ein abschließender Zeilenumbruch wird ignoriert, und beides zugleich zu setzen ist ein Fehler.
- **Beim Start geprüft.** Fehlt eine Einstellung oder ist sie ungültig, startet das Gateway nicht. Seine Meldung nennt die Einstellung und gibt nie ein Secret aus.

| Typ | Werte |
| --- | --- |
| Text | Text, URLs, Adressen (`0.0.0.0:8080`), Dauern (`10s`, `5m`, `24h`), Größen (`32MiB`, `1MB` oder eine Anzahl Bytes) und die aufgeführten Auswahlwerte |
| Bool | `true` oder `false`; auch `1` und `0`, `yes` und `no`, `on` und `off` |
| Ganzzahl | Eine ganze Zahl, null oder größer |

## Einstellungen

### Adressen

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_LISTEN`<br>`listen` | Text | `0.0.0.0:8080` | Adresse der Modell-API, der Platform API und der Health-Checks. |
| `KAESO_METRICS_LISTEN`<br>`metrics_listen` | Text | `127.0.0.1:9100` | Adresse der Prometheus-Metriken. Nie öffentlich weiterleiten. Das Gateway-Image setzt `0.0.0.0:9100`, damit ein aus dem Container veröffentlichter Port sie erreicht; [Zustand und Metriken](monitoring.md#metriken) zeigt, wie. |
| `KAESO_PUBLIC_URL`<br>`public_url` | Text | erforderlich | Die Adresse, die Personen und Apps verwenden, etwa `https://kaeso.example.com`, ohne Pfad. Sie muss `https` sein, außer `http` auf einer Loopback-Adresse für die Entwicklung. |
| `KAESO_TRUSTED_PROXIES`<br>`trusted_proxies` | Text | `127.0.0.0/8,::1/128` | Kommagetrennte Netze von Proxys, deren `X-Forwarded-For` das Gateway vertraut. `trusted_proxies = ""` in der TOML-Datei vertraut keinem; eine leere Variable behält den Standard. |
| `KAESO_SHUTDOWN_GRACE`<br>`shutdown_grace` | Text | `30s` | Wie lange laufende Anfragen und Streams nach dem Signal zum Beenden noch laufen dürfen. Was dann noch offen ist, endet mit 503 und wird abgerechnet und protokolliert. |
| `KAESO_SERVER_HEADER_READ_TIMEOUT`<br>`server.header_read_timeout` | Text | `30s` | Wie lange ein Client für die Header einer Anfrage brauchen oder eine Verbindung zwischen Anfragen ruhen lassen darf; 1s bis 10m. Eine laufende Antwort bricht es nie ab. |
| `KAESO_SERVER_BODY_READ_TIMEOUT`<br>`server.body_read_timeout` | Text | `30s` | Wie lange das Gateway auf den nächsten Teil eines Anfrage-Bodys wartet, bevor es die Anfrage mit 400 ablehnt; 1s bis 10m. |
| `KAESO_SERVER_BODY_READ_DEADLINE`<br>`server.body_read_deadline` | Text | `60s` | Wie lange das Gateway auf einen ganzen Anfrage-Body wartet, wie gleichmäßig seine Teile auch ankommen, bevor es die Anfrage mit 400 ablehnt; 1s bis 1h. Ein Body von 8 MiB, die Standardgrenze der Modell-API, kommt bei 1,2 Mbit/s rechtzeitig an; erhöhen Sie den Wert für Apps mit langsameren Verbindungen. |
| `KAESO_SERVER_MAX_CONNECTIONS`<br>`server.max_connections` | Ganzzahl | `4096` | Verbindungen, die das Gateway gleichzeitig bedient, von Apps oder einem vorgeschalteten Proxy. Weitere warten in der Warteschlange des Betriebssystems, bis eine schließt. Der Standard entspricht der Standard-Warteschlange von Linux, `net.core.somaxconn`. |

Auf einer Loopback-Adresse mit `http`, etwa `http://localhost:8080`, kommt das Sitzungscookie ohne `Secure` und ohne das Präfix `__Host-` aus, sodass die Konsole auf Ihrem Rechner über einfaches HTTP funktioniert.

### Browser-Apps

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_CORS_ALLOWED_ORIGINS`<br>`cors.allowed_origins` | Text | — | Kommagetrennte Origins von Web-Apps, die die Modell-API aus dem Browser aufrufen dürfen, etwa `https://app.example.com,http://localhost:5173`. Nur diese erhalten CORS-Header, nur unter `/v1`, und mit `*` startet das Gateway nicht. |

Ohne diese Einstellung sendet Kaeso keine CORS-Header. Die [Modell-API](../api/model-api.md) erklärt, wie eine Browser-App Kaeso aufruft.

### Datenbanken

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_POSTGRES_URL`<br>`postgres.url` | Text, Secret | erforderlich | Verbindungs-URL für PostgreSQL. |
| `KAESO_POSTGRES_POOL_SIZE`<br>`postgres.pool_size` | Ganzzahl | `20` | Höchstzahl der PostgreSQL-Verbindungen je Gateway-Instanz, mindestens 1. Hintergrundjobs halten ihre Sperren zusätzlich über einige weitere Verbindungen. |
| `KAESO_CLICKHOUSE_URL`<br>`clickhouse.url` | Text, Secret | — | HTTP-URL für ClickHouse mit Benutzer und Passwort, etwa `http://kaeso:secret@clickhouse:8123`. Nicht gesetzt oder leer schaltet die Anfrageprotokolle aus. |
| `KAESO_CLICKHOUSE_DATABASE`<br>`clickhouse.database` | Text | `kaeso` | ClickHouse-Datenbank für die Anfrageprotokolle, aus Buchstaben, Ziffern und `_`. Das Gateway legt sie an, wenn sie fehlt. |
| `KAESO_VALKEY_URL`<br>`valkey.url` | Text, Secret | erforderlich | URL für Valkey (oder Redis) mit dem Passwort, etwa `redis://:secret@valkey:6379`. |
| `KAESO_VALKEY_KEY_PREFIX`<br>`valkey.key_prefix` | Text | `kaeso:` | Präfix aller Valkey-Schlüssel und -Kanäle, um eine Valkey-Instanz zwischen Installationen zu teilen, die einander vertrauen. |

### Hauptschlüssel

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_MASTER_KEYS`<br>`master_keys` | Text, Secret | erforderlich | Hauptschlüssel als `id:base64,…`, der aktive zuerst. `kaeso master-key generate` erzeugt einen ([Backups und Hauptschlüssel](backups.md)). |

### E-Mail

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_SMTP_HOST`<br>`smtp.host` | Text | — | SMTP-Server. Erforderlich, außer `mail.transport` ist `log`. |
| `KAESO_SMTP_PORT`<br>`smtp.port` | Ganzzahl | `587` | SMTP-Port. |
| `KAESO_SMTP_TLS`<br>`smtp.tls` | Text | `starttls` | `starttls`, `tls` für implizites TLS (meist Port 465) oder `none` nur für einen lokalen Mail-Catcher. |
| `KAESO_SMTP_USER`<br>`smtp.user` | Text | — | SMTP-Benutzername, falls der Server einen braucht. Erforderlich mit einem Passwort. |
| `KAESO_SMTP_PASSWORD`<br>`smtp.password` | Text, Secret | — | SMTP-Passwort. |
| `KAESO_SMTP_FROM`<br>`smtp.from` | Text | `Kaeso <kaeso@localhost>` | Absender jeder E-Mail. `compose.yaml` setzt `Kaeso <kaeso@` und Ihre Domain. |
| `KAESO_MAIL_TRANSPORT`<br>`mail.transport` | Text | `smtp` | `smtp` oder `log`, um nur die Art jeder E-Mail zu protokollieren, nie Empfänger oder Inhalt. |

### Anmeldung

Diese Einstellungen schalten Passwörter und Anmeldelinks für die ganze Installation ein oder aus. Die Konsole zeigt sie an, und eine hier abgeschaltete Methode lässt sich dort nicht einschalten. Für Single Sign-on gibt es keine Einstellung der Installation: Es funktioniert für eine Organisation, sobald ihr Admin einen Identitätsanbieter verbindet ([Anmeldung und Single Sign-on](sign-in.md)).

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_AUTH_PASSWORD`<br>`auth.password` | Bool | `true` | Anmeldung mit E-Mail und Passwort erlauben. |
| `KAESO_AUTH_MAGIC_LINK`<br>`auth.magic_link` | Bool | `true` | Anmeldung mit einem Link per E-Mail erlauben. |
| `KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS`<br>`auth.oidc_allow_private_networks` | Bool | `false` | OIDC-Anfragen an private und Loopback-Adressen erlauben, für einen Identitätsanbieter in Ihrem eigenen Netz. Schalten Sie es nur ein, wo Admins von Organisationen Ihr internes Netz über das Gateway erreichen dürfen. Link-lokale Adressen und Metadaten-Adressen von Clouds wie `169.254.169.254` bleiben gesperrt. |

### Fehlgeschlagene Schlüsselprüfungen

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_KEY_FAILURES_PER_ADDRESS`<br>`key_failures.per_address` | Ganzzahl | `100` | Erlaubte fehlgeschlagene Schlüssel-Abfragen je Client-Adresse und Zeitfenster, über beide APIs zusammen. `0` schaltet die Drosselung ab ([Drosselung von API-Schlüsseln](#drosselung-von-api-schlüsseln)). |
| `KAESO_KEY_FAILURES_WINDOW`<br>`key_failures.window` | Text | `1m` | Das Zeitfenster von `key_failures.per_address`, 1s bis 1h. Eine gesperrte Adresse wartet höchstens so lange. |

### models.dev

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_MODELS_DEV_SYNC`<br>`models_dev.sync` | Bool | `true` | Modelldaten von models.dev abrufen. Ohne sie verwendet das Gateway die Daten, die im Release enthalten sind. |
| `KAESO_MODELS_DEV_URL`<br>`models_dev.url` | Text | `https://models.dev/api.json` | Woher die Modelldaten kommen. Die Adresse muss `https` sein, außer `http` auf einer Loopback-Adresse. Kaeso folgt höchstens 3 Weiterleitungen, jede auf demselben Host und über `https`. |
| `KAESO_MODELS_DEV_INTERVAL`<br>`models_dev.interval` | Text | `24h` | Wie oft das Gateway sie abruft. |

### Updates

Die ersten drei Einstellungen legen nur die Anfangswerte einer neuen Installation fest. Danach ändern Instanz-Admins sie unter **Instanz** › **Updates** ([Upgrades](upgrades.md)).

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_UPDATES_CHECK`<br>`updates.check` | Bool | `true` | Alle 5 Minuten nach neuen Kaeso-Releases suchen. |
| `KAESO_UPDATES_CHANNEL`<br>`updates.channel` | Text | `stable` | Die angebotenen Releases: `stable` oder `prerelease`, um auch Vorabversionen zu erhalten. |
| `KAESO_UPDATES_MODE`<br>`updates.mode` | Text | `manual` | Wie eine neue Installation Updates über den Updater erhält: `manual`, wobei ein Instanz-Admin jedes startet, oder `automatic`, im Update-Fenster. |
| `KAESO_UPDATES_RELEASES_URL`<br>`updates.releases_url` | Text | `https://update.kaeso.app/v1/releases.json` | Woher die Liste der Releases kommt: update.kaeso.app oder ein Spiegel, der wie die Releases-API von GitHub antwortet. |
| `KAESO_UPDATER_URL`<br>`updater.url` | Text | `http://updater:8081` | Adresse des Updaters. |
| `KAESO_UPDATER_TOKEN`<br>`updater.token` | Text, Secret | — | Das Token, das Gateway und Updater teilen, mindestens 32 Zeichen. Ohne es ruft das Gateway den Updater nie auf. |

### Aufrufe an Anbieter

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_PROVIDERS_OPENAI_BASE_URL`<br>`providers.openai.base_url` | Text | `https://api.openai.com` | Basis-URL der OpenAI-API. |
| `KAESO_PROVIDERS_ANTHROPIC_BASE_URL`<br>`providers.anthropic.base_url` | Text | `https://api.anthropic.com` | Basis-URL der Anthropic-API. |
| `KAESO_PROVIDERS_DEEPSEEK_BASE_URL`<br>`providers.deepseek.base_url` | Text | `https://api.deepseek.com` | Basis-URL der DeepSeek-API. |
| `KAESO_PROVIDERS_XAI_BASE_URL`<br>`providers.xai.base_url` | Text | `https://api.x.ai` | Basis-URL der SpaceX-API. |
| `KAESO_PROVIDERS_TOGETHERAI_BASE_URL`<br>`providers.togetherai.base_url` | Text | `https://api.together.ai` | Basis-URL der Together-AI-API. |
| `KAESO_PROVIDERS_GROQ_BASE_URL`<br>`providers.groq.base_url` | Text | `https://api.groq.com/openai` | Basis-URL der API von Groq. |
| `KAESO_PROVIDERS_OPENROUTER_BASE_URL`<br>`providers.openrouter.base_url` | Text | `https://openrouter.ai/api` | Basis-URL der API von OpenRouter. |
| `KAESO_PROVIDERS_MISTRAL_BASE_URL`<br>`providers.mistral.base_url` | Text | `https://api.mistral.ai` | Basis-URL der Mistral-API. |
| `KAESO_PROVIDERS_VERCEL_BASE_URL`<br>`providers.vercel.base_url` | Text | `https://ai-gateway.vercel.sh` | Basis-URL der Vercel-API. |
| `KAESO_PROVIDERS_COHERE_BASE_URL`<br>`providers.cohere.base_url` | Text | `https://api.cohere.ai/compatibility` | Basis-URL der Cohere-API. |
| `KAESO_PROVIDERS_OLLAMA_CLOUD_BASE_URL`<br>`providers.ollama-cloud.base_url` | Text | `https://ollama.com` | Basis-URL der Ollama-Cloud-API. |
| `KAESO_PROVIDERS_MOONSHOTAI_BASE_URL`<br>`providers.moonshotai.base_url` | Text | `https://api.moonshot.ai` | Basis-URL der Moonshot-AI-API. |
| `KAESO_PROVIDERS_ALIBABA_BASE_URL`<br>`providers.alibaba.base_url` | Text | `https://ap-southeast-1.maas.qwencloudapi.com/compatible-mode` | Basis-URL der QwenCloud-API. |
| `KAESO_UPSTREAM_CONNECT_TIMEOUT`<br>`upstream.connect_timeout` | Text | `10s` | Zeit für den Verbindungsaufbau zu einem Anbieter, TCP und TLS; höchstens 24h. |
| `KAESO_UPSTREAM_REQUEST_TIMEOUT`<br>`upstream.request_timeout` | Text | `600s` | Gesamtzeit einer Anbieter-Anfrage ohne Streaming; höchstens 24h. |
| `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT`<br>`upstream.first_byte_timeout` | Text | `60s` | Streaming: Zeit bis zum ersten Byte des Anbieters; höchstens 24h. |
| `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT`<br>`upstream.stream_idle_timeout` | Text | `300s` | Streaming: längste Stille zwischen zwei Chunks; höchstens 24h. |

Ein Modell im Katalog eines Projekts kann eigene Zeitlimits festlegen ([Modelle und Aliasse](../console/models-and-aliases.md)).

Das Gateway ruft Anbieter über den Proxy in `HTTPS_PROXY` auf (oder `HTTP_PROXY` für `http`-Basis-URLs, oder `ALL_PROXY`), wenn seine Umgebung einen setzt, außer für die Hosts in `NO_PROXY`. Der Proxy sieht die Anbieter-Schlüssel von `http`-Aufrufen, verwenden Sie über ihn also `https`-Basis-URLs. Eigene Anbieter sind die Ausnahme: Das Gateway ruft sie direkt auf ([Eigene Anbieter](#eigene-anbieter)).

### Eigene Anbieter

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_CUSTOM_PROVIDERS_ENABLED`<br>`custom_providers.enabled` | bool | `true` | Organisations-Admins dürfen eigene Anbieter hinzufügen, etwa ein selbst betriebenes vLLM oder einen internen Proxy ([Eigene Anbieter](../console/custom-providers.md)). `false` lehnt Anfragen an die hinzugefügten ab und behält sie. |
| `KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS`<br>`custom_providers.allowed_networks` | text | — | Kommagetrennte private, Loopback- oder andere nicht öffentliche Netze, die eigene Anbieter erreichen dürfen, etwa `10.0.0.0/8` oder `127.0.0.1/32`. Reines `http` erreicht nur diese. |

Die Basis-URL eines eigenen Anbieters wählt ein Organisations-Admin, also überwacht das Gateway, wohin es sich verbindet. Für jede Verbindung löst es den Namen des Hosts auf und prüft jede Adresse. Private, Loopback-, Link-Local- und Cloud-Metadaten-Adressen lehnt es ab, außer `KAESO_CUSTOM_PROVIDERS_ALLOWED_NETWORKS` nennt ihr Netz. Löst ein Name später zu einer anderen Adresse auf, wird sie bei der nächsten Verbindung erneut geprüft. Link-Local- und Cloud-Metadaten-Adressen wie `169.254.169.254` bleiben abgelehnt, auch wenn ein genanntes Netz sie umfasst.

Das Gateway folgt außerdem keinen Weiterleitungen eines eigenen Anbieters und ruft ihn ohne den Proxy in `HTTPS_PROXY` auf, weil ein Proxy den Namen an diesen Prüfungen vorbei auflösen würde. Erlauben Sie ein Netz nur, wo jeder Organisations-Admin die Server darin über das Gateway erreichen darf. Für ein vLLM auf demselben Host wie die Container des Gateways nennen Sie das Docker-Netz, etwa `172.16.0.0/12`.

### Streams an Apps

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_STREAMS_KEEPALIVE_INTERVAL`<br>`streams.keepalive_interval` | Text | `15s` | Nach so langer Zeit ohne Ausgabe ein Keepalive in einen offenen Stream schreiben, damit Proxys die Verbindung halten ([Lange Streams](domains-and-tls.md#lange-streams)). `0s` schaltet Keepalives ab; sonst 1s bis 300s. |
| `KAESO_STREAMS_MAX_DURATION`<br>`streams.max_duration` | Text | `0s` | Wie lange eine gestreamte Antwort höchstens laufen darf, bevor das Gateway sie mit einem Fehler-Event beendet. `0s` setzt keine Grenze. |
| `KAESO_STREAMS_MAX_BYTES`<br>`streams.max_bytes` | Text | `0` | Wie viele Bytes, etwa `64MiB`, eine gestreamte Antwort höchstens an die App schreiben darf, bevor das Gateway sie mit einem Fehler-Event beendet. `0` setzt keine Grenze. |

### Kontinuität des Reasonings

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_CONTINUITY_REASONING_TTL`<br>`continuity.reasoning_ttl` | Text | `1h` | Wie lange das Gateway das Reasoning eines Modells vor einem Tool-Aufruf aufbewahrt, für eine OpenAI-Chat-Completions-App, deren Modell ein anderes Format spricht. `0s` schaltet es ab; sonst 1m bis 24h. |

[Formatübersetzung](../concepts/format-translation.md#chat-completions) erklärt, wie das Gateway es aufbewahrt.

### Anfrage- und Antwortkörper

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_BODY_LIMITS_MODEL_API`<br>`body_limits.model_api` | Text | `8MiB` | Größter Anfragekörper der Modell-API. Er fasst ein Bild an der 5-MB-Grenze von Anthropic als Base64; für große PDFs erhöhen Sie ihn. |
| `KAESO_BODY_LIMITS_IN_FLIGHT`<br>`body_limits.in_flight` | Text | `512MiB` | Speicher, den Modell-API und Playground gleichzeitig zum Übersetzen von Anfragen nutzen dürfen, geschätzt aus jedem Körper, wie er ankommt, und aus seinen Nachrichten, Blöcken und Werkzeugen. Eine Anfrage hält ihre Schätzung, während sie decodiert und codiert wird, danach bis zu ihrem Ende nur, was belegt bleibt: ihre decodierte Anfrage samt Nachrichten, Blöcken, Werkzeugen und Token-IDs, eine zweite Kopie davon, solange der Anbieter noch gebeten werden kann, ihre Eingabe-Token zu zählen, den an den Anbieter gesendeten Körper, bis er antwortet, und ihren rohen Körper, solange Inhalte protokolliert werden. Ein Schlüssel hält höchstens ein Viertel, und eine größere Anfrage nimmt das Viertel. Eine Anfrage, die binnen 10 Sekunden keinen Platz findet, erhält 503 mit `Retry-After: 10`. |
| `KAESO_BODY_LIMITS_PLATFORM_API`<br>`body_limits.platform_api` | Text | `1MiB` | Größter Anfragekörper der Platform API, Anfragen des Playground eingeschlossen. |
| `KAESO_BODY_LIMITS_PROVIDER_RESPONSE`<br>`body_limits.provider_response` | Text | `128MiB` | Größter Antwortkörper, den das Gateway ohne Streaming von einem Anbieter liest. Ein größerer lässt die Anfrage mit 502 scheitern. |

### Anfrageprotokolle

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_LOGS_BUFFER_SIZE`<br>`logs.buffer_size` | Text | `256MiB` | Speicher für Zeilen der Anfrageprotokolle, die auf ClickHouse warten. Darüber hinaus fallen die ältesten Zeilen weg. |
| `KAESO_LOGS_FLUSH_INTERVAL`<br>`logs.flush_interval` | Text | `1s` | Wie oft das Gateway wartende Zeilen nach ClickHouse schreibt. |
| `KAESO_LOGS_FLUSH_ROWS`<br>`logs.flush_rows` | Ganzzahl | `5000` | Wartende Zeilen schreiben, sobald so viele warten, mindestens 1. |

### Standards der Instanz

Diese Einstellungen legen nur die Anfangswerte einer neuen Installation fest. Danach ändern Instanz-Admins sie über die Platform API oder in der Konsole: unter **Instanz** › **Allgemein**, die Aufbewahrung unter **Instanz** › **Protokollierung**. Die gespeicherten Werte haben Vorrang.

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_INSTANCE_ORG_CREATION`<br>`instance.org_creation` | Text | `instance_admins` | Wer Organisationen anlegen darf: `instance_admins` oder `everyone`. |
| `KAESO_INSTANCE_ACCOUNT_CREATION`<br>`instance.account_creation` | Text | `invitation_only` | Wer ohne Einladung ein Konto anlegen darf: `invitation_only` oder `everyone`. |
| `KAESO_INSTANCE_METADATA_RETENTION_DAYS`<br>`instance.metadata_retention_days` | Ganzzahl | `90` | Tage, die neue Organisationen Metadaten der Anfrageprotokolle aufbewahren, mindestens 1 und höchstens das Maximum unten. |
| `KAESO_INSTANCE_CONTENT_RETENTION_DAYS`<br>`instance.content_retention_days` | Ganzzahl | `30` | Tage, die neue Organisationen Inhalte der Anfrageprotokolle aufbewahren, mindestens 1 und nie länger als die Metadaten. |
| `KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS`<br>`instance.max_metadata_retention_days` | Ganzzahl | `3650` | Längste Aufbewahrung von Metadaten, die eine Organisation wählen darf, höchstens 36500 Tage. |
| `KAESO_INSTANCE_MAX_CONTENT_RETENTION_DAYS`<br>`instance.max_content_retention_days` | Ganzzahl | `3650` | Längste Aufbewahrung von Inhalten, die eine Organisation wählen darf, höchstens 36500 Tage. |
| `KAESO_INSTANCE_ORG_DELETION`<br>`instance.org_deletion` | Bool | `true` | Ob sich Organisationen löschen lassen. |
| `KAESO_INSTANCE_ORG_DELETION_GRACE_DAYS`<br>`instance.org_deletion_grace_days` | Ganzzahl | `7` | Tage, die eine gelöschte Organisation wiederherstellbar und schreibgeschützt bleibt, bevor sie endgültig entfernt wird, 1 bis 90. |
| `KAESO_INSTANCE_TIMEZONE`<br>`instance.timezone` | Text | `UTC` | Die IANA-Zeitzone einer neuen Installation, etwa `Europe/Berlin`: Die Konsole zeigt jede Zeit darin, und automatische Updates richten sich danach. Die Einrichtungsseite setzt sie auf die Zeitzone des ersten Benutzers; Instanz-Admins ändern sie später in den Instanzeinstellungen. |

In der Konsole können Instanz-Admins das Anlegen von Konten und Organisationen außerdem auf bestimmte E-Mail-Domains beschränken.

### Logging

| Variable<br>TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- |
| `KAESO_LOG_LEVEL`<br>`log.level` | Text | `info,sqlx=warn` | Log-Level oder Filter, etwa `info` oder `info,kaeso_server=debug`. |
| `KAESO_LOG_FORMAT`<br>`log.format` | Text | `json` | `json` für strukturierte Logs, `text` für die Entwicklung. |

## Anbietereinstellungen

Jeder Anbieter hat eigene Einstellungen, benannt nach seiner ID: den TOML-Schlüssel `providers.<id>.<field>` und die Variable `KAESO_PROVIDERS_<ID>_<FIELD>`, mit der ID in Großbuchstaben und jedem `-` als `_`. Jeder Anbieter, den Kaeso aufruft, hat ein Feld, `base_url` ([Aufrufe an Anbieter](#aufrufe-an-anbieter)).

Eine Anbieter-ID besteht aus Kleinbuchstaben und Ziffern mit einzelnen Bindestrichen dazwischen. Eine Anbieter-Variable oder -Tabelle mit einer anderen ID, oder mit der ID eines Anbieters, den Kaeso nicht aufruft, hält das Gateway beim Start an. Die Meldung nennt sie und listet die Anbieter auf.

## Drosselung von API-Schlüsseln

Das Gateway zählt fehlgeschlagene Abfragen von API-Schlüsseln je Client-Adresse in Valkey, damit ein Client, der Schlüssel errät, PostgreSQL nicht beschäftigt hält. Nach `KAESO_KEY_FAILURES_PER_ADDRESS` Fehlschlägen innerhalb von `KAESO_KEY_FAILURES_WINDOW` ist die Adresse bis zum Ende des Zeitfensters gesperrt. Jeder Schlüssel von dort, der nicht im Speicher liegt, erhält dann 429 mit `Retry-After`, im Format der jeweiligen API.

- Es zählen nur wohlgeformte Schlüssel, die zu keinem Schlüssel passen. Ein fehlerhafter Wert, ein deaktivierter oder abgelaufener Schlüssel und ein unbekannter Schlüssel, der binnen 30 Sekunden wiederkommt, zählen nicht.
- Gültige Schlüssel zählen nie. Ein Schlüssel, den die Instanz in den letzten 30 Sekunden verwendet hat, funktioniert auch von einer gesperrten Adresse; andere gültige Schlüssel warten das Zeitfenster ab.
- IPv6-Adressen zählen je /64.
- Hinter einem Proxy kommt die Client-Adresse nur dann aus `X-Forwarded-For`, wenn der Proxy in `KAESO_TRUSTED_PROXIES` steht. Sonst teilen sich alle Clients das Kontingent des Proxys.
- Docker Desktop, Colima und rootless Docker leiten veröffentlichte Ports über einen eigenen Proxy weiter. Caddy sieht dann jeden Client unter einer Adresse, und alle Clients teilen sich ein Kontingent. Damit das Limit je Client gilt, betreiben Sie Kaeso auf Docker Engine mit Root-Netzwerk.
- Solange Valkey nicht erreichbar ist, werden Fehlschläge nicht gezählt.

Jede Sperre erhöht `kaeso_key_failure_blocks_total` und protokolliert `too many failed API key lookups from one address` mit der Adresse ([Zustand und Metriken](monitoring.md)).
