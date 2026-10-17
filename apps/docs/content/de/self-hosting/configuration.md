---
title: Konfiguration
description: Wie das Gateway seine Einstellungen liest, und jede Einstellung, die es hat.
order: 3
---

Das Gateway wird mit `KAESO_*`-Umgebungsvariablen konfiguriert, optional ergänzt durch eine TOML-Datei. In der Compose-Installation gehören die Variablen, die `compose.yaml` liest, in `.env`, wie [die Datei .env](installation.md#die-datei-env) sie auflistet; jede andere Einstellung gehört in `compose.override.yaml`, wie [lokale Änderungen](installation.md#lokale-änderungen) zeigt. Eine Einstellung, die nur in `.env` steht, erreicht das Gateway nie. Die Compose-Datei setzt bereits die Datenbank-URLs, die Datei mit den Hauptschlüsseln und das JSON-Logformat und leitet die öffentliche URL aus `KAESO_DOMAIN` ab.

## Wie Einstellungen gelesen werden

- **Umgebungsvariablen.** Jede Einstellung hat eine Variable, etwa `KAESO_LOG_LEVEL`. Eine leere Variable gilt als nicht gesetzt.
- **Eine Datei.** `KAESO_CONFIG=/etc/kaeso/kaeso.toml` liest Einstellungen aus einer TOML-Datei, mit den TOML-Schlüsseln unten als Tabellen geschrieben (`[postgres]`, dann `url = "…"`). Eine Umgebungsvariable hat Vorrang vor der Datei. Ein Schlüssel, der keine Einstellung ist, ist ein Fehler. Jedes Release enthält [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml); die Datei setzt jede Einstellung, die kein Secret ist, auf ihren Standard oder, wo es keinen gibt, auf einen Beispielwert und ist damit ein gültiger Ausgangspunkt.
- **Secrets aus Dateien.** Jede als Secret markierte Einstellung lässt sich stattdessen aus einer Datei lesen, die in `<VARIABLE>_FILE` genannt ist, etwa `KAESO_MASTER_KEYS_FILE=/run/secrets/master_keys` für Docker-Secrets. Ein abschließender Zeilenumbruch wird ignoriert. Die Variable und ihr `_FILE` zugleich zu setzen ist ein Fehler.
- **Beim Start geprüft.** Das Gateway prüft beim Start jede Einstellung und startet nicht, mit einer Meldung, die die Einstellung nennt. Geheime Werte gibt es nie aus.

| Typ | Werte |
| --- | --- |
| Text | Text, URLs, Adressen (`0.0.0.0:8080`), Dauern (`10s`, `5m`, `24h`), Größen (`32MiB`, `1MB` oder eine Anzahl Bytes) und die aufgeführten Auswahlwerte |
| Bool | `true` oder `false`; auch `1` und `0`, `yes` und `no`, `on` und `off` |
| Ganzzahl | Eine ganze Zahl, null oder größer |

## Einstellungen

### Listener

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_LISTEN` | `listen` | Text | `0.0.0.0:8080` | Adresse der Modell-API, der Platform API und der Health-Checks. |
| `KAESO_METRICS_LISTEN` | `metrics_listen` | Text | `0.0.0.0:9100` | Adresse des Prometheus-Metrik-Listeners. Nie öffentlich weiterleiten. |
| `KAESO_PUBLIC_URL` | `public_url` | Text | erforderlich | Die URL, über die Personen und Apps diese Installation erreichen, etwa `https://kaeso.example.com`: der reine Origin, ohne Pfad. Muss `https` sein, außer `http://localhost` für die Entwicklung. |
| `KAESO_TRUSTED_PROXIES` | `trusted_proxies` | Text | `127.0.0.0/8,::1/128,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7` | Kommagetrennte Netzwerke von Reverse Proxys, deren `X-Forwarded-For` geglaubt wird, für Client-Adressen in Logs, die Drosselung von Anmeldungen und die Drosselung von API-Schlüsseln. `trusted_proxies = ""` in der TOML-Datei vertraut keinem Proxy; eine leere Variable behält den Standard. |
| `KAESO_SHUTDOWN_GRACE` | `shutdown_grace` | Text | `30s` | Wie lange laufende Anfragen und Streams nach einem Shutdown-Signal weiterlaufen dürfen. Die dann noch offenen enden mit einem Fehler im Format der App (503) und werden abgerechnet und protokolliert. |
| `KAESO_SERVER_HEADER_READ_TIMEOUT` | `server.header_read_timeout` | Text | `30s` | Wie lange ein Client für die Header einer Anfrage brauchen darf und wie lange eine Verbindung zwischen zwei Anfragen ungenutzt bleiben darf, bevor das Gateway sie schließt; 1 s bis 10 min. Eine Anfrage, die gerade beantwortet wird, etwa ein langer Stream, wird dadurch nie unterbrochen. |

### Browser-Apps

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_CORS_ALLOWED_ORIGINS` | `cors.allowed_origins` | Text | — | Kommagetrennte Origins der Web-Apps, die die Modell-API aus einem Browser aufrufen dürfen, etwa `https://app.example.com,http://localhost:5173`: Schema, Host und Port, ohne Pfad. Nur sie bekommen CORS-Header, und nur auf `/v1`; ist keiner eingetragen, sendet Kaeso überhaupt keine CORS-Header. `*` und Werte, die kein Origin sind, verhindern den Start des Gateways. Siehe [Aufrufe aus dem Browser](../api/model-api.md#aufrufe-aus-dem-browser). |

### Datenbanken

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_POSTGRES_URL` | `postgres.url` | Text, Secret | erforderlich | Verbindungs-URL von PostgreSQL. |
| `KAESO_POSTGRES_POOL_SIZE` | `postgres.pool_size` | Ganzzahl | `20` | Höchstzahl der PostgreSQL-Verbindungen pro Gateway-Instanz, mindestens 1. Hintergrundjobs halten ihre Sperren zusätzlich auf eigenen, kurzlebigen Verbindungen. |
| `KAESO_CLICKHOUSE_URL` | `clickhouse.url` | Text, Secret | erforderlich | HTTP-URL von ClickHouse, mit Benutzer und Passwort, etwa `http://kaeso:secret@clickhouse:8123`. |
| `KAESO_CLICKHOUSE_DATABASE` | `clickhouse.database` | Text | `kaeso` | ClickHouse-Datenbank für Anfrageprotokolle, aus Buchstaben, Ziffern und `_`; wird angelegt, wenn sie fehlt. |
| `KAESO_VALKEY_URL` | `valkey.url` | Text, Secret | erforderlich | URL von Valkey (oder Redis), mit dem Passwort, wenn es eines verlangt, etwa `redis://:secret@valkey:6379`. |
| `KAESO_VALKEY_KEY_PREFIX` | `valkey.key_prefix` | Text | `kaeso:` | Präfix jedes Valkey-Schlüssels und -Kanals, um eine Valkey-Instanz zwischen Installationen zu teilen. |

### Hauptschlüssel

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_MASTER_KEYS` | `master_keys` | Text, Secret | erforderlich | Hauptschlüssel als `id:base64,…`, der aktive Schlüssel zuerst. Einen mit `kaeso master-key generate` erzeugen. Siehe [Backups und Hauptschlüssel](backups.md). |

### E-Mail

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_SMTP_HOST` | `smtp.host` | Text | — | SMTP-Server. Erforderlich, außer `mail.transport` ist `log`. |
| `KAESO_SMTP_PORT` | `smtp.port` | Ganzzahl | `587` | SMTP-Port. |
| `KAESO_SMTP_TLS` | `smtp.tls` | Text | `starttls` | `starttls`, `tls` (implizites TLS, meist Port 465) oder `none` (nur für einen lokalen Mail-Catcher). |
| `KAESO_SMTP_USER` | `smtp.user` | Text | — | SMTP-Benutzername, falls der Server einen braucht. Erforderlich, wenn ein Passwort gesetzt ist. |
| `KAESO_SMTP_PASSWORD` | `smtp.password` | Text, Secret | — | SMTP-Passwort. |
| `KAESO_SMTP_FROM` | `smtp.from` | Text | `Kaeso <kaeso@localhost>` | Absender jeder E-Mail, die Kaeso sendet. Die Compose-Datei setzt `Kaeso <kaeso@` und die eigene Domain. |
| `KAESO_MAIL_TRANSPORT` | `mail.transport` | Text | `smtp` | `smtp`, oder `log`, um nur zu protokollieren, dass eine E-Mail gesendet worden wäre, mit ihrer Art, nie mit Empfänger oder Inhalt. Zum Ausprobieren von Kaeso und für die Entwicklung. |

### Anmeldung

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_AUTH_PASSWORD` | `auth.password` | Bool | `true` | Anmeldung mit E-Mail-Adresse und Passwort erlauben. |
| `KAESO_AUTH_MAGIC_LINK` | `auth.magic_link` | Bool | `true` | Anmeldung mit einem Anmeldelink per E-Mail erlauben. |
| `KAESO_AUTH_OIDC` | `auth.oidc` | Bool | `true` | Anmeldung über den OIDC-Identitätsanbieter einer Organisation erlauben. |
| `KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS` | `auth.oidc_allow_private_networks` | Bool | `true` | OIDC-Discovery- und Token-Anfragen an private und Loopback-Adressen erlauben. Auf Installationen abschalten, auf denen Organisations-Admins das interne Netzwerk nicht über das Gateway erreichen sollen. |

### Fehlgeschlagene API-Schlüssel

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_KEY_FAILURES_PER_ADDRESS` | `key_failures.per_address` | Ganzzahl | `100` | Erlaubte fehlgeschlagene Schlüsselsuchen pro Client-Adresse und Zeitfenster, auf Modell-API und Platform API zusammen; darüber hinaus werden nicht gecachte Schlüssel bis zum Ende des Fensters mit 429 abgelehnt. `0` schaltet die Drosselung ab. Siehe [Drosselung von API-Schlüsseln](#drosselung-von-api-schlüsseln). |
| `KAESO_KEY_FAILURES_WINDOW` | `key_failures.window` | Text | `1m` | Das Zeitfenster von `key_failures.per_address`, 1s bis 1h. Eine gesperrte Adresse wartet höchstens so lange. |

### models.dev

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_MODELS_DEV_SYNC` | `models_dev.sync` | Bool | `true` | models.dev regelmäßig abrufen. Für Installationen ohne Internetzugang abschalten; dann wird der im Release enthaltene Stand genutzt. |
| `KAESO_MODELS_DEV_URL` | `models_dev.url` | Text | `https://models.dev/api.json` | Woher die Daten von models.dev abgerufen werden. |
| `KAESO_MODELS_DEV_INTERVAL` | `models_dev.interval` | Text | `24h` | Wie oft models.dev abgerufen wird. |

### Aufrufe an Anbieter

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_PROVIDERS_OPENAI_BASE_URL` | `providers.openai.base_url` | Text | `https://api.openai.com` | Basis-URL der OpenAI-API. |
| `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` | `providers.anthropic.base_url` | Text | `https://api.anthropic.com` | Basis-URL der Anthropic-API. |
| `KAESO_UPSTREAM_CONNECT_TIMEOUT` | `upstream.connect_timeout` | Text | `10s` | Verbindungs-Timeout zu Anbietern, TCP und TLS; höchstens 24h. |
| `KAESO_UPSTREAM_REQUEST_TIMEOUT` | `upstream.request_timeout` | Text | `600s` | Gesamt-Timeout einer Anfrage an einen Anbieter, die nicht streamt; höchstens 24h. |
| `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | `upstream.first_byte_timeout` | Text | `60s` | Streaming: Zeit bis zum ersten Byte vom Anbieter; höchstens 24h. |
| `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | `upstream.stream_idle_timeout` | Text | `300s` | Streaming: längste Stille zwischen zwei Chunks; höchstens 24h. |

Timeouts für ein einzelnes Modell werden am Modell im Katalog eines Projekts festgelegt; siehe [Modelle und Aliasse](../console/models-and-aliases.md).

### Streams an Apps

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_STREAMS_KEEPALIVE_INTERVAL` | `streams.keepalive_interval` | Text | `15s` | Solange eine gestreamte Antwort offen ist und so lange nichts an die App geschrieben wurde, ein Keepalive schreiben, das das SDK der App ignoriert (ein SSE-Kommentar für die OpenAI-Formate, ein `ping`-Event für Anthropic), damit Proxys mit Idle-Timeouts die Verbindung halten. `0s` schaltet Keepalives ab; sonst 1s bis 300s. Siehe [lange Streams](domains-and-tls.md#lange-streams). |

### Kontinuität des Reasonings

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_CONTINUITY_REASONING_TTL` | `continuity.reasoning_ttl` | Text | `1h` | Wie lange das Gateway das Reasoning vor einem Tool-Aufruf für eine OpenAI-Chat-Completions-App aufbewahrt, deren Modell in einem anderen Format aufgerufen wird (Chat Completions hat dafür kein Feld), damit die nächste Runde der App es zurückbringt. In Valkey gehalten, mit dem Hauptschlüssel verschlüsselt und nur mit dem API-Schlüssel lesbar, der die Anfrage gestellt hat; nie protokolliert. `0s` schaltet es ab; sonst 1m bis 24h. Siehe [Formatübersetzung](../concepts/format-translation.md). |

### Anfrage- und Antwortkörper

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_BODY_LIMITS_MODEL_API` | `body_limits.model_api` | Text | `32MiB` | Größter Anfragekörper der Modell-API. |
| `KAESO_BODY_LIMITS_PLATFORM_API` | `body_limits.platform_api` | Text | `1MiB` | Größter Anfragekörper der Platform API. |
| `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | `body_limits.provider_response` | Text | `128MiB` | Größter Antwortkörper ohne Streaming, den Kaeso von einem Anbieter liest; ein größerer lässt die Anfrage mit 502 scheitern. |

### Anfrageprotokolle

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_LOGS_BUFFER_SIZE` | `logs.buffer_size` | Text | `256MiB` | Speicher für Zeilen der Anfrageprotokolle, die auf ClickHouse warten; darüber hinaus werden die ältesten Zeilen verworfen. |
| `KAESO_LOGS_FLUSH_INTERVAL` | `logs.flush_interval` | Text | `1s` | Wie oft gepufferte Protokollzeilen nach ClickHouse geschrieben werden. |
| `KAESO_LOGS_FLUSH_ROWS` | `logs.flush_rows` | Ganzzahl | `5000` | Gepufferte Protokollzeilen schreiben, sobald so viele warten, mindestens 1. |

### Standards der Instanz

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_INSTANCE_ORG_CREATION` | `instance.org_creation` | Text | `instance_admins` | Wer auf einer neuen Installation Organisationen erstellen darf: `instance_admins` oder `everyone`. Instanz-Admins ändern das später in den Instanzeinstellungen, wo sie auch Benutzer mit bestimmten E-Mail-Domains zulassen können. |
| `KAESO_INSTANCE_ACCOUNT_CREATION` | `instance.account_creation` | Text | `invitation_only` | Wer auf einer neuen Installation ohne Einladung ein Konto erstellen darf: `invitation_only` oder `everyone`. Instanz-Admins ändern das später in den Instanzeinstellungen, wo sie auch nur bestimmte E-Mail-Domains zulassen können. |
| `KAESO_INSTANCE_METADATA_RETENTION_DAYS` | `instance.metadata_retention_days` | Ganzzahl | `90` | Standardaufbewahrung der Metadaten von Anfrageprotokollen für neue Organisationen, in Tagen; mindestens 1 und höchstens der Höchstwert unten. |
| `KAESO_INSTANCE_CONTENT_RETENTION_DAYS` | `instance.content_retention_days` | Ganzzahl | `30` | Standardaufbewahrung der Inhalte von Anfrageprotokollen für neue Organisationen, in Tagen; mindestens 1, nie länger als die Aufbewahrung der Metadaten und höchstens der Höchstwert unten. |
| `KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS` | `instance.max_metadata_retention_days` | Ganzzahl | `3650` | Längste Aufbewahrung der Metadaten, die eine Organisation wählen darf, in Tagen; höchstens 36500. |
| `KAESO_INSTANCE_MAX_CONTENT_RETENTION_DAYS` | `instance.max_content_retention_days` | Ganzzahl | `3650` | Längste Aufbewahrung der Inhalte, die eine Organisation wählen darf, in Tagen; höchstens 36500. |
| `KAESO_INSTANCE_ORG_DELETION` | `instance.org_deletion` | Bool | `true` | Ob Organisationen auf einer neuen Installation gelöscht werden können. Instanz-Admins ändern das später in den Instanzeinstellungen. |
| `KAESO_INSTANCE_ORG_DELETION_GRACE_DAYS` | `instance.org_deletion_grace_days` | Ganzzahl | `7` | Tage, die eine gelöschte Organisation wiederherstellbar und schreibgeschützt bleibt, bevor sie endgültig entfernt wird, 1 bis 90. |

### Logging

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_LOG_LEVEL` | `log.level` | Text | `info,sqlx=warn` | Log-Level oder Filter, etwa `info` oder `info,kaeso_server=debug`. |
| `KAESO_LOG_FORMAT` | `log.format` | Text | `json` | `json` für strukturierte Logs, `text` für die Entwicklung. |

## Anbietereinstellungen

Einstellungen eines Anbieters werden einmal pro Anbieter geschrieben: als TOML-Schlüssel `providers.<id>.<feld>` und als Variable `KAESO_PROVIDERS_<ID>_<FELD>`. Die Anbieter sind `openai` und `anthropic`, und jeder hat ein Feld, `base_url`, wie [Aufrufe an Anbieter](#aufrufe-an-anbieter) zeigt.

- **Anbieter-IDs** bestehen aus Kleinbuchstaben und Ziffern, mit einzelnen Bindestrichen dazwischen (`^[a-z0-9]+(-[a-z0-9]+)*$`): keine Großbuchstaben, Unterstriche oder Punkte. Dieselbe Regel gilt überall, wo eine ID vorkommt, auch für Anbieter-Schlüssel und Modelle im Katalog in der Konsole und der Platform API.
- **Die Variable** ist `KAESO_PROVIDERS_`, die ID in Großbuchstaben mit `_` für jeden `-`, dann `_` und das Feld in Großbuchstaben. Ein Anbieter mit der ID `azure-openai` würde mit `KAESO_PROVIDERS_AZURE_OPENAI_BASE_URL` gesetzt, in der Datei mit `[providers.azure-openai]` und `base_url = "…"`.
- **Eine Bedeutung pro Variable.** Kaeso legt die Felder fest, und kein Feldname endet auf `_` und den Namen eines anderen Felds; so liest das Gateway eine Variable, indem es das Feld am Ende abtrennt, und der Rest ergibt genau eine ID.
- **Unbekannte Anbieter werden abgelehnt.** Eine Variable `KAESO_PROVIDERS_…` oder eine Tabelle `[providers.…]`, deren ID die Regel verletzt oder einen Anbieter nennt, den Kaeso nicht aufruft, hält das Gateway beim Start an, mit einer Meldung, die sie nennt und die Anbieter aufzählt.

## Drosselung von API-Schlüsseln

Eine Anfrage mit einem API-Schlüssel, den das Gateway nicht kürzlich gesehen hat, kostet eine Abfrage in PostgreSQL. Unbekannte Schlüssel werden 30 Sekunden lang gemerkt, sodass eine App, die weiter einen rotierten Schlüssel sendet, eine Abfrage pro 30 Sekunden und Gateway-Instanz kostet; ein Client, der für jede Anfrage einen neuen Schlüssel erfindet, kostete eine pro Anfrage. Deshalb zählt das Gateway fehlgeschlagene Schlüsselsuchen pro Client-Adresse, auf Modell-API und Platform API zusammen, in Valkey. Nach `KAESO_KEY_FAILURES_PER_ADDRESS` Fehlschlägen (100) innerhalb von `KAESO_KEY_FAILURES_WINDOW` (1 Minute) ist die Adresse bis zum Ende des Fensters gesperrt: Jeder nicht gecachte Schlüssel von ihr erhält 429 mit `Retry-After`, im eigenen Format der API, ohne Datenbankabfrage.

- Nur Suchen, die keinen Schlüssel finden, zählen. Fehlerhaft geformte Werte, deaktivierte oder abgelaufene Schlüssel und ein unbekannter Schlüssel, der innerhalb von 30 Sekunden wiederholt wird, zählen nicht.
- Gültige Schlüssel zählen nie. Ein Schlüssel, der in den letzten 30 Sekunden auf einer Instanz genutzt wurde, wird aus dem Speicher beantwortet, auch von einer gesperrten Adresse; ein gültiger Schlüssel, der länger nicht genutzt wurde, wartet auf das Ende des Fensters.
- IPv6-Adressen zählen pro /64.
- Hinter einem Reverse Proxy kommt die Client-Adresse nur dann aus `X-Forwarded-For`, wenn der Proxy in `KAESO_TRUSTED_PROXIES` steht. Mit einem nicht vertrauenswürdigen Proxy davor sieht jeder Client wie der Proxy aus und teilt dessen Kontingent, daher die eigenen Proxys eintragen.
- Ist Valkey nicht erreichbar, werden Fehlschläge nicht gezählt.

Die Metrik `kaeso_key_failure_blocks_total` zählt Sperren, und die Logzeile `too many failed API key lookups from one address` nennt die Adresse; siehe [Zustand und Metriken](monitoring.md).

## Hinweise

- **Standards der Instanz** füllen nur die Instanzeinstellungen einer neuen Installation vor. Danach ändern Instanz-Admins, wer Konten und Organisationen erstellen darf, ob Organisationen gelöscht werden können und deren Karenzzeit sowie die Standards und Höchstwerte der Aufbewahrung unter **Einstellungen** › **Instanz** › **Einstellungen** in der Konsole oder über die Platform API (`instance/settings`); die gespeicherten Werte haben Vorrang.
- **Anmeldemethoden**, die hier abgeschaltet sind, lassen sich in der Konsole nicht einschalten; sie zeigt sie so, wie die Konfiguration des Gateways sie festlegt.
- **Entwicklung.** Mit `KAESO_PUBLIC_URL=http://localhost:…` wird das Sitzungscookie ohne `Secure` und ohne das Präfix `__Host-` gesendet, sodass die Konsole auf dem eigenen Rechner über einfaches HTTP funktioniert. Jede andere `http`-URL wird abgelehnt.
