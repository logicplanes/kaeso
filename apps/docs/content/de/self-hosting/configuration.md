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
| `CLICKHOUSE_PASSWORD` | ja | Das Passwort des ClickHouse-Benutzers `kaeso`. |
| `VALKEY_PASSWORD` | ja | Das Passwort, das Valkey von jeder Verbindung verlangt. |
| `KAESO_PUBLIC_URL` | nein | Die öffentliche URL, wenn sie nicht `https://` und die Domain ist, etwa hinter Ihrem eigenen Proxy. |
| `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | `KAESO_SMTP_HOST`, sofern E-Mails nicht nur protokolliert werden | Wie das Gateway E-Mails sendet ([E-Mail](#e-mail)). |
| `KAESO_MAIL_TRANSPORT` | nein | `log`, um E-Mails nur zu protokollieren statt sie zu senden. |
| `KAESO_UPDATER_TOKEN`, `COMPOSE_PROFILES` | nein | Das Token, das Gateway und Updater teilen, und `updater`, um den Updater zu starten ([Der Updater](upgrades.md#der-updater)). |
| `DOCKER_SOCKET` | nein | Der Pfad des Docker-Sockets für den Updater, wenn er nicht `/var/run/docker.sock` ist. |

Verwenden Sie in den Passwörtern nur Buchstaben und Ziffern, wie `openssl rand -hex` sie schreibt, denn sie stehen in Verbindungs-URLs. PostgreSQL behält die Passwörter, die es beim Anlegen seines Volumes hatte; ändern Sie `POSTGRES_PASSWORD` später, sperrt das das Gateway aus. ClickHouse und Valkey übernehmen ihr Passwort bei jedem Start aus der `.env`.

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

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_LISTEN` | `listen` | Text | `0.0.0.0:8080` | Adresse der Modell-API, der Platform API und der Health-Checks. |
| `KAESO_METRICS_LISTEN` | `metrics_listen` | Text | `0.0.0.0:9100` | Adresse der Prometheus-Metriken. Nie öffentlich weiterleiten. |
| `KAESO_PUBLIC_URL` | `public_url` | Text | erforderlich | Die Adresse, die Personen und Apps verwenden, etwa `https://kaeso.example.com`, ohne Pfad. Sie muss `https` sein, außer `http` auf einer Loopback-Adresse für die Entwicklung. |
| `KAESO_TRUSTED_PROXIES` | `trusted_proxies` | Text | `127.0.0.0/8,::1/128,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7` | Kommagetrennte Netze von Proxys, deren `X-Forwarded-For` das Gateway vertraut. `trusted_proxies = ""` in der TOML-Datei vertraut keinem; eine leere Variable behält den Standard. |
| `KAESO_SHUTDOWN_GRACE` | `shutdown_grace` | Text | `30s` | Wie lange laufende Anfragen und Streams nach dem Signal zum Beenden noch laufen dürfen. Was dann noch offen ist, endet mit 503 und wird abgerechnet und protokolliert. |
| `KAESO_SERVER_HEADER_READ_TIMEOUT` | `server.header_read_timeout` | Text | `30s` | Wie lange ein Client für die Header einer Anfrage brauchen oder eine Verbindung zwischen Anfragen ruhen lassen darf; 1s bis 10m. Eine laufende Antwort bricht es nie ab. |

Auf einer Loopback-Adresse mit `http`, etwa `http://localhost:8080`, kommt das Sitzungscookie ohne `Secure` und ohne das Präfix `__Host-` aus, sodass die Konsole auf Ihrem Rechner über einfaches HTTP funktioniert.

### Browser-Apps

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_CORS_ALLOWED_ORIGINS` | `cors.allowed_origins` | Text | — | Kommagetrennte Origins von Web-Apps, die die Modell-API aus dem Browser aufrufen dürfen, etwa `https://app.example.com,http://localhost:5173`. Nur diese erhalten CORS-Header, nur unter `/v1`, und mit `*` startet das Gateway nicht. |

Ohne diese Einstellung sendet Kaeso keine CORS-Header. Die [Modell-API](../api/model-api.md) erklärt, wie eine Browser-App Kaeso aufruft.

### Datenbanken

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_POSTGRES_URL` | `postgres.url` | Text, Secret | erforderlich | Verbindungs-URL für PostgreSQL. |
| `KAESO_POSTGRES_POOL_SIZE` | `postgres.pool_size` | Ganzzahl | `20` | Höchstzahl der PostgreSQL-Verbindungen je Gateway-Instanz, mindestens 1. Hintergrundjobs halten ihre Sperren zusätzlich über einige weitere Verbindungen. |
| `KAESO_CLICKHOUSE_URL` | `clickhouse.url` | Text, Secret | erforderlich | HTTP-URL für ClickHouse mit Benutzer und Passwort, etwa `http://kaeso:secret@clickhouse:8123`. |
| `KAESO_CLICKHOUSE_DATABASE` | `clickhouse.database` | Text | `kaeso` | ClickHouse-Datenbank für die Anfrageprotokolle, aus Buchstaben, Ziffern und `_`. Das Gateway legt sie an, wenn sie fehlt. |
| `KAESO_VALKEY_URL` | `valkey.url` | Text, Secret | erforderlich | URL für Valkey (oder Redis) mit dem Passwort, etwa `redis://:secret@valkey:6379`. |
| `KAESO_VALKEY_KEY_PREFIX` | `valkey.key_prefix` | Text | `kaeso:` | Präfix aller Valkey-Schlüssel und -Kanäle, um eine Valkey-Instanz zwischen Installationen zu teilen. |

### Hauptschlüssel

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_MASTER_KEYS` | `master_keys` | Text, Secret | erforderlich | Hauptschlüssel als `id:base64,…`, der aktive zuerst. `kaeso master-key generate` erzeugt einen ([Backups und Hauptschlüssel](backups.md)). |

### E-Mail

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_SMTP_HOST` | `smtp.host` | Text | — | SMTP-Server. Erforderlich, außer `mail.transport` ist `log`. |
| `KAESO_SMTP_PORT` | `smtp.port` | Ganzzahl | `587` | SMTP-Port. |
| `KAESO_SMTP_TLS` | `smtp.tls` | Text | `starttls` | `starttls`, `tls` für implizites TLS (meist Port 465) oder `none` nur für einen lokalen Mail-Catcher. |
| `KAESO_SMTP_USER` | `smtp.user` | Text | — | SMTP-Benutzername, falls der Server einen braucht. Erforderlich mit einem Passwort. |
| `KAESO_SMTP_PASSWORD` | `smtp.password` | Text, Secret | — | SMTP-Passwort. |
| `KAESO_SMTP_FROM` | `smtp.from` | Text | `Kaeso <kaeso@localhost>` | Absender jeder E-Mail. `compose.yaml` setzt `Kaeso <kaeso@` und Ihre Domain. |
| `KAESO_MAIL_TRANSPORT` | `mail.transport` | Text | `smtp` | `smtp` oder `log`, um nur die Art jeder E-Mail zu protokollieren, nie Empfänger oder Inhalt. |

### Anmeldung

Diese Einstellungen schalten Anmeldemethoden für die ganze Installation ein oder aus. Die Konsole zeigt sie an, und eine hier abgeschaltete Methode lässt sich dort nicht einschalten ([Anmeldung und Single Sign-on](sign-in.md)).

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_AUTH_PASSWORD` | `auth.password` | Bool | `true` | Anmeldung mit E-Mail und Passwort erlauben. |
| `KAESO_AUTH_MAGIC_LINK` | `auth.magic_link` | Bool | `true` | Anmeldung mit einem Link per E-Mail erlauben. |
| `KAESO_AUTH_OIDC` | `auth.oidc` | Bool | `true` | Anmeldung über den OIDC-Identitätsanbieter einer Organisation erlauben. |
| `KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS` | `auth.oidc_allow_private_networks` | Bool | `true` | OIDC-Anfragen an private und Loopback-Adressen erlauben. Schalten Sie es ab, wo Admins von Organisationen Ihr internes Netz nicht über das Gateway erreichen sollen. |

### Fehlgeschlagene Schlüsselprüfungen

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_KEY_FAILURES_PER_ADDRESS` | `key_failures.per_address` | Ganzzahl | `100` | Erlaubte fehlgeschlagene Schlüssel-Abfragen je Client-Adresse und Zeitfenster, über beide APIs zusammen. `0` schaltet die Drosselung ab ([Drosselung von API-Schlüsseln](#drosselung-von-api-schlüsseln)). |
| `KAESO_KEY_FAILURES_WINDOW` | `key_failures.window` | Text | `1m` | Das Zeitfenster von `key_failures.per_address`, 1s bis 1h. Eine gesperrte Adresse wartet höchstens so lange. |

### models.dev

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_MODELS_DEV_SYNC` | `models_dev.sync` | Bool | `true` | Modelldaten von models.dev abrufen. Ohne sie verwendet das Gateway die Daten, die im Release enthalten sind. |
| `KAESO_MODELS_DEV_URL` | `models_dev.url` | Text | `https://models.dev/api.json` | Woher die Modelldaten kommen. |
| `KAESO_MODELS_DEV_INTERVAL` | `models_dev.interval` | Text | `24h` | Wie oft das Gateway sie abruft. |

### Updates

Die ersten drei Einstellungen legen nur die Anfangswerte einer neuen Installation fest. Danach ändern Instanz-Admins sie unter **Einstellungen** › **Instanz** › **Updates** ([Upgrades](upgrades.md)).

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_UPDATES_CHECK` | `updates.check` | Bool | `true` | Zweimal am Tag nach neuen Kaeso-Releases suchen. |
| `KAESO_UPDATES_CHANNEL` | `updates.channel` | Text | `stable` | Die angebotenen Releases: `stable` oder `prerelease`, um auch Vorabversionen zu erhalten. |
| `KAESO_UPDATES_MODE` | `updates.mode` | Text | `manual` | Wie eine neue Installation Updates über den Updater erhält: `manual`, wobei ein Instanz-Admin jedes startet, oder `automatic`, im Update-Fenster. |
| `KAESO_UPDATES_RELEASES_URL` | `updates.releases_url` | Text | `https://update.kaeso.app/v1/releases.json` | Woher die Liste der Releases kommt: update.kaeso.app oder ein Spiegel, der wie die Releases-API von GitHub antwortet. |
| `KAESO_UPDATER_URL` | `updater.url` | Text | `http://updater:8081` | Adresse des Updaters. |
| `KAESO_UPDATER_TOKEN` | `updater.token` | Text, Secret | — | Das Token, das Gateway und Updater teilen, mindestens 32 Zeichen. Ohne es ruft das Gateway den Updater nie auf. |

### Aufrufe an Anbieter

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_PROVIDERS_OPENAI_BASE_URL` | `providers.openai.base_url` | Text | `https://api.openai.com` | Basis-URL der OpenAI-API. |
| `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` | `providers.anthropic.base_url` | Text | `https://api.anthropic.com` | Basis-URL der Anthropic-API. |
| `KAESO_PROVIDERS_TOGETHERAI_BASE_URL` | `providers.togetherai.base_url` | Text | `https://api.together.ai` | Basis-URL der Together-AI-API. |
| `KAESO_UPSTREAM_CONNECT_TIMEOUT` | `upstream.connect_timeout` | Text | `10s` | Zeit für den Verbindungsaufbau zu einem Anbieter, TCP und TLS; höchstens 24h. |
| `KAESO_UPSTREAM_REQUEST_TIMEOUT` | `upstream.request_timeout` | Text | `600s` | Gesamtzeit einer Anbieter-Anfrage ohne Streaming; höchstens 24h. |
| `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | `upstream.first_byte_timeout` | Text | `60s` | Streaming: Zeit bis zum ersten Byte des Anbieters; höchstens 24h. |
| `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | `upstream.stream_idle_timeout` | Text | `300s` | Streaming: längste Stille zwischen zwei Chunks; höchstens 24h. |

Ein Modell im Katalog eines Projekts kann eigene Zeitlimits festlegen ([Modelle und Aliasse](../console/models-and-aliases.md)).

### Streams an Apps

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_STREAMS_KEEPALIVE_INTERVAL` | `streams.keepalive_interval` | Text | `15s` | Nach so langer Zeit ohne Ausgabe ein Keepalive in einen offenen Stream schreiben, damit Proxys die Verbindung halten ([Lange Streams](domains-and-tls.md#lange-streams)). `0s` schaltet Keepalives ab; sonst 1s bis 300s. |

### Kontinuität des Reasonings

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_CONTINUITY_REASONING_TTL` | `continuity.reasoning_ttl` | Text | `1h` | Wie lange das Gateway das Reasoning eines Modells vor einem Tool-Aufruf aufbewahrt, für eine OpenAI-Chat-Completions-App, deren Modell ein anderes Format spricht. `0s` schaltet es ab; sonst 1m bis 24h. |

[Formatübersetzung](../concepts/format-translation.md#chat-completions) erklärt, wie das Gateway es aufbewahrt.

### Anfrage- und Antwortkörper

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_BODY_LIMITS_MODEL_API` | `body_limits.model_api` | Text | `32MiB` | Größter Anfragekörper der Modell-API. |
| `KAESO_BODY_LIMITS_PLATFORM_API` | `body_limits.platform_api` | Text | `1MiB` | Größter Anfragekörper der Platform API. |
| `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | `body_limits.provider_response` | Text | `128MiB` | Größter Antwortkörper, den das Gateway ohne Streaming von einem Anbieter liest. Ein größerer lässt die Anfrage mit 502 scheitern. |

### Anfrageprotokolle

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_LOGS_BUFFER_SIZE` | `logs.buffer_size` | Text | `256MiB` | Speicher für Zeilen der Anfrageprotokolle, die auf ClickHouse warten. Darüber hinaus fallen die ältesten Zeilen weg. |
| `KAESO_LOGS_FLUSH_INTERVAL` | `logs.flush_interval` | Text | `1s` | Wie oft das Gateway wartende Zeilen nach ClickHouse schreibt. |
| `KAESO_LOGS_FLUSH_ROWS` | `logs.flush_rows` | Ganzzahl | `5000` | Wartende Zeilen schreiben, sobald so viele warten, mindestens 1. |

### Standards der Instanz

Diese Einstellungen legen nur die Anfangswerte einer neuen Installation fest. Danach ändern Instanz-Admins sie unter **Einstellungen** › **Instanz** › **Einstellungen** oder über die Platform API, und die gespeicherten Werte haben Vorrang.

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_INSTANCE_ORG_CREATION` | `instance.org_creation` | Text | `instance_admins` | Wer Organisationen anlegen darf: `instance_admins` oder `everyone`. |
| `KAESO_INSTANCE_ACCOUNT_CREATION` | `instance.account_creation` | Text | `invitation_only` | Wer ohne Einladung ein Konto anlegen darf: `invitation_only` oder `everyone`. |
| `KAESO_INSTANCE_METADATA_RETENTION_DAYS` | `instance.metadata_retention_days` | Ganzzahl | `90` | Tage, die neue Organisationen Metadaten der Anfrageprotokolle aufbewahren, mindestens 1 und höchstens das Maximum unten. |
| `KAESO_INSTANCE_CONTENT_RETENTION_DAYS` | `instance.content_retention_days` | Ganzzahl | `30` | Tage, die neue Organisationen Inhalte der Anfrageprotokolle aufbewahren, mindestens 1 und nie länger als die Metadaten. |
| `KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS` | `instance.max_metadata_retention_days` | Ganzzahl | `3650` | Längste Aufbewahrung von Metadaten, die eine Organisation wählen darf, höchstens 36500 Tage. |
| `KAESO_INSTANCE_MAX_CONTENT_RETENTION_DAYS` | `instance.max_content_retention_days` | Ganzzahl | `3650` | Längste Aufbewahrung von Inhalten, die eine Organisation wählen darf, höchstens 36500 Tage. |
| `KAESO_INSTANCE_ORG_DELETION` | `instance.org_deletion` | Bool | `true` | Ob sich Organisationen löschen lassen. |
| `KAESO_INSTANCE_ORG_DELETION_GRACE_DAYS` | `instance.org_deletion_grace_days` | Ganzzahl | `7` | Tage, die eine gelöschte Organisation wiederherstellbar und schreibgeschützt bleibt, bevor sie endgültig entfernt wird, 1 bis 90. |
| `KAESO_INSTANCE_TIMEZONE` | `instance.timezone` | Text | `UTC` | Die IANA-Zeitzone einer neuen Installation, etwa `Europe/Berlin`: Die Konsole zeigt jede Zeit darin, und automatische Updates richten sich danach. Die Einrichtungsseite setzt sie auf die Zeitzone des ersten Benutzers; Instanz-Admins ändern sie später in den Instanzeinstellungen. |

In der Konsole können Instanz-Admins das Anlegen von Konten und Organisationen außerdem auf bestimmte E-Mail-Domains beschränken.

### Logging

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_LOG_LEVEL` | `log.level` | Text | `info,sqlx=warn` | Log-Level oder Filter, etwa `info` oder `info,kaeso_server=debug`. |
| `KAESO_LOG_FORMAT` | `log.format` | Text | `json` | `json` für strukturierte Logs, `text` für die Entwicklung. |

## Anbietereinstellungen

Jeder Anbieter hat eigene Einstellungen, benannt nach seiner ID: den TOML-Schlüssel `providers.<id>.<field>` und die Variable `KAESO_PROVIDERS_<ID>_<FIELD>`, mit der ID in Großbuchstaben und jedem `-` als `_`. Kaeso ruft drei Anbieter auf, `openai`, `anthropic` und `togetherai`, und jeder hat ein Feld, `base_url` ([Aufrufe an Anbieter](#aufrufe-an-anbieter)).

Eine Anbieter-ID besteht aus Kleinbuchstaben und Ziffern mit einzelnen Bindestrichen dazwischen. Eine Anbieter-Variable oder -Tabelle mit einer anderen ID, oder mit der ID eines Anbieters, den Kaeso nicht aufruft, hält das Gateway beim Start an. Die Meldung nennt sie und listet die Anbieter auf.

## Drosselung von API-Schlüsseln

Das Gateway zählt fehlgeschlagene Abfragen von API-Schlüsseln je Client-Adresse in Valkey, damit ein Client, der Schlüssel errät, PostgreSQL nicht beschäftigt hält. Nach `KAESO_KEY_FAILURES_PER_ADDRESS` Fehlschlägen innerhalb von `KAESO_KEY_FAILURES_WINDOW` ist die Adresse bis zum Ende des Zeitfensters gesperrt. Jeder Schlüssel von dort, der nicht im Speicher liegt, erhält dann 429 mit `Retry-After`, im Format der jeweiligen API.

- Es zählen nur wohlgeformte Schlüssel, die zu keinem Schlüssel passen. Ein fehlerhafter Wert, ein deaktivierter oder abgelaufener Schlüssel und ein unbekannter Schlüssel, der binnen 30 Sekunden wiederkommt, zählen nicht.
- Gültige Schlüssel zählen nie. Ein Schlüssel, den die Instanz in den letzten 30 Sekunden verwendet hat, funktioniert auch von einer gesperrten Adresse; andere gültige Schlüssel warten das Zeitfenster ab.
- IPv6-Adressen zählen je /64.
- Hinter einem Proxy kommt die Client-Adresse nur dann aus `X-Forwarded-For`, wenn der Proxy in `KAESO_TRUSTED_PROXIES` steht. Sonst teilen sich alle Clients das Kontingent des Proxys.
- Solange Valkey nicht erreichbar ist, werden Fehlschläge nicht gezählt.

Jede Sperre erhöht `kaeso_key_failure_blocks_total` und protokolliert `too many failed API key lookups from one address` mit der Adresse ([Zustand und Metriken](monitoring.md)).
