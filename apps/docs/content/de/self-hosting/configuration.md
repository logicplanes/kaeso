---
title: Konfiguration
description: Wie das Gateway seine Einstellungen liest, und jede Einstellung, die es hat.
order: 3
---

Das Gateway liest seine Einstellungen aus `KAESO_*`-Umgebungsvariablen, optional ergänzt durch eine TOML-Datei. In der Compose-Installation gilt:

- Die Variablen, die `compose.yaml` liest, stehen in der `.env`; [Die Datei .env](installation.md#die-datei-env) listet sie auf.
- Jede andere Einstellung gehört in die `compose.override.yaml`, wie [Lokale Änderungen](installation.md#lokale-änderungen) zeigt. Steht sie nur in der `.env`, erreicht sie das Gateway nie.
- `compose.yaml` setzt bereits die Datenbank-URLs, die Datei mit den Hauptschlüsseln und das JSON-Logformat und leitet `KAESO_PUBLIC_URL` aus `KAESO_DOMAIN` ab.

## Wie Einstellungen gelesen werden

- **Umgebungsvariablen.** Jede Einstellung hat eine, etwa `KAESO_LOG_LEVEL`. Eine leere Variable gilt als nicht gesetzt.
- **Eine TOML-Datei.** `KAESO_CONFIG=/etc/kaeso/kaeso.toml` liest Einstellungen aus einer Datei, mit den Schlüsseln unten als Tabellen geschrieben (`[postgres]`, dann `url = "…"`). Eine Umgebungsvariable hat Vorrang vor der Datei, und ein Schlüssel, der keine Einstellung ist, ist ein Fehler. [`kaeso.example.toml`](https://github.com/logicplanes/kaeso/releases/latest/download/kaeso.example.toml), die jedem Release beiliegt, setzt jede Einstellung, die kein Secret ist, auf ihren Standard oder einen Beispielwert und ist ein gültiger Ausgangspunkt.
- **Secrets aus Dateien.** Jede geheime Einstellung lässt sich stattdessen aus der Datei lesen, die `<VARIABLE>_FILE` nennt, etwa `KAESO_MASTER_KEYS_FILE=/run/secrets/master_keys` für ein Docker-Secret. Ein abschließender Zeilenumbruch wird ignoriert; die Variable und ihr `_FILE` zugleich zu setzen ist ein Fehler.
- **Beim Start geprüft.** Fehlt eine Einstellung oder ist sie ungültig, startet das Gateway nicht und nennt die Einstellung in seiner Meldung, ohne je ein Secret auszugeben.

| Typ | Werte |
| --- | --- |
| Text | Text, URLs, Adressen (`0.0.0.0:8080`), Dauern (`10s`, `5m`, `24h`), Größen (`32MiB`, `1MB` oder eine Anzahl Bytes) und die aufgeführten Auswahlwerte |
| Bool | `true` oder `false`; auch `1` und `0`, `yes` und `no`, `on` und `off` |
| Ganzzahl | Eine ganze Zahl, null oder größer |

## Einstellungen

### Listener

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_LISTEN` | `listen` | Text | `0.0.0.0:8080` | Adresse für Modell-API, Platform API und Health-Checks. |
| `KAESO_METRICS_LISTEN` | `metrics_listen` | Text | `0.0.0.0:9100` | Adresse, unter der die Prometheus-Metriken bereitstehen. Nie öffentlich weiterleiten. |
| `KAESO_PUBLIC_URL` | `public_url` | Text | erforderlich | Die URL, unter der Personen und Apps diese Installation erreichen, etwa `https://kaeso.example.com`: nur der Origin, ohne Pfad. Muss `https` sein; für die Entwicklung ist `http` auf einer Loopback-Adresse wie `http://localhost` erlaubt. |
| `KAESO_TRUSTED_PROXIES` | `trusted_proxies` | Text | `127.0.0.0/8,::1/128,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7` | Kommagetrennte Netze von Reverse Proxys, deren `X-Forwarded-For` das Gateway vertraut; daraus ergibt sich die Client-Adresse in Logs und für die Drosselung von Anmeldungen und API-Schlüsseln. `trusted_proxies = ""` in der TOML-Datei vertraut keinem Proxy; eine leere Variable behält den Standard. |
| `KAESO_SHUTDOWN_GRACE` | `shutdown_grace` | Text | `30s` | Wie lange laufende Anfragen und Streams nach dem Signal zum Beenden noch weiterlaufen dürfen. Was dann noch offen ist, endet mit einem Fehler im Format der App (503) und wird abgerechnet und protokolliert. |
| `KAESO_SERVER_HEADER_READ_TIMEOUT` | `server.header_read_timeout` | Text | `30s` | Wie lange ein Client für die Header einer Anfrage brauchen und eine Verbindung zwischen zwei Anfragen ruhen darf, bevor das Gateway sie schließt; 1s bis 10m. Eine Anfrage, die gerade beantwortet wird, etwa ein langer Stream, bricht es dadurch nie ab. |

### Browser-Apps

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_CORS_ALLOWED_ORIGINS` | `cors.allowed_origins` | Text | — | Kommagetrennte Origins der Web-Apps, die die Modell-API aus dem Browser aufrufen dürfen, etwa `https://app.example.com,http://localhost:5173`: Schema, Host und Port, ohne Pfad. Nur diese erhalten CORS-Header, und nur unter `/v1`; ohne Eintrag sendet Kaeso gar keine CORS-Header. Mit `*` oder einem Wert, der kein Origin ist, startet das Gateway nicht. Siehe [Aufrufe aus dem Browser](../api/model-api.md#aufrufe-aus-dem-browser). |

### Datenbanken

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_POSTGRES_URL` | `postgres.url` | Text, Secret | erforderlich | Verbindungs-URL für PostgreSQL. |
| `KAESO_POSTGRES_POOL_SIZE` | `postgres.pool_size` | Ganzzahl | `20` | Höchstzahl der PostgreSQL-Verbindungen je Gateway-Instanz, mindestens 1. Hintergrundjobs halten ihre Sperren zusätzlich über eigene, kurzlebige Verbindungen. |
| `KAESO_CLICKHOUSE_URL` | `clickhouse.url` | Text, Secret | erforderlich | HTTP-URL für ClickHouse mit Benutzer und Passwort, etwa `http://kaeso:secret@clickhouse:8123`. |
| `KAESO_CLICKHOUSE_DATABASE` | `clickhouse.database` | Text | `kaeso` | ClickHouse-Datenbank für die Anfrageprotokolle, nur aus Buchstaben, Ziffern und `_`; wird angelegt, falls sie fehlt. |
| `KAESO_VALKEY_URL` | `valkey.url` | Text, Secret | erforderlich | URL für Valkey (oder Redis), mit Passwort, falls verlangt, etwa `redis://:secret@valkey:6379`. |
| `KAESO_VALKEY_KEY_PREFIX` | `valkey.key_prefix` | Text | `kaeso:` | Präfix aller Valkey-Schlüssel und -Kanäle, damit sich mehrere Installationen eine Valkey-Instanz teilen können. |

### Hauptschlüssel

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_MASTER_KEYS` | `master_keys` | Text, Secret | erforderlich | Hauptschlüssel als `id:base64,…`, der aktive zuerst. Mit `kaeso master-key generate` erzeugen. Siehe [Backups und Hauptschlüssel](backups.md). |

### E-Mail

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_SMTP_HOST` | `smtp.host` | Text | — | SMTP-Server. Pflicht, außer `mail.transport` ist `log`. |
| `KAESO_SMTP_PORT` | `smtp.port` | Ganzzahl | `587` | SMTP-Port. |
| `KAESO_SMTP_TLS` | `smtp.tls` | Text | `starttls` | `starttls`, `tls` (implizites TLS, meist Port 465) oder `none` (nur für einen lokalen Mail-Catcher). |
| `KAESO_SMTP_USER` | `smtp.user` | Text | — | SMTP-Benutzername, falls der Server einen verlangt. Pflicht, sobald ein Passwort gesetzt ist. |
| `KAESO_SMTP_PASSWORD` | `smtp.password` | Text, Secret | — | SMTP-Passwort. |
| `KAESO_SMTP_FROM` | `smtp.from` | Text | `Kaeso <kaeso@localhost>` | Absender aller E-Mails von Kaeso. Die Compose-Datei setzt `Kaeso <kaeso@` und die eigene Domain. |
| `KAESO_MAIL_TRANSPORT` | `mail.transport` | Text | `smtp` | `smtp` oder `log`; mit `log` protokolliert das Gateway nur, dass eine E-Mail gesendet worden wäre, mit ihrer Art, nie mit Empfänger oder Inhalt. Zum Ausprobieren und für die Entwicklung. |

### Anmeldung

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_AUTH_PASSWORD` | `auth.password` | Bool | `true` | Anmeldung mit E-Mail-Adresse und Passwort zulassen. |
| `KAESO_AUTH_MAGIC_LINK` | `auth.magic_link` | Bool | `true` | Anmeldung mit einem Anmeldelink per E-Mail zulassen. |
| `KAESO_AUTH_OIDC` | `auth.oidc` | Bool | `true` | Anmeldung über den OIDC-Identitätsanbieter einer Organisation zulassen. |
| `KAESO_AUTH_OIDC_ALLOW_PRIVATE_NETWORKS` | `auth.oidc_allow_private_networks` | Bool | `true` | OIDC-Discovery- und Token-Anfragen an private und Loopback-Adressen zulassen. Abschalten, wo Organisations-Admins das interne Netz nicht über das Gateway erreichen sollen. |

### Fehlgeschlagene API-Schlüssel

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_KEY_FAILURES_PER_ADDRESS` | `key_failures.per_address` | Ganzzahl | `100` | Zulässige fehlgeschlagene Schlüsselabfragen je Client-Adresse und Zeitfenster, über Modell-API und Platform API zusammen; darüber hinaus lehnt das Gateway Schlüssel, die nicht im Cache liegen, bis zum Ende des Fensters mit 429 ab. `0` schaltet die Drosselung ab. Siehe [Drosselung von API-Schlüsseln](#drosselung-von-api-schlüsseln). |
| `KAESO_KEY_FAILURES_WINDOW` | `key_failures.window` | Text | `1m` | Zeitfenster für `key_failures.per_address`, 1s bis 1h. Länger wartet eine gesperrte Adresse nie. |

### models.dev

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_MODELS_DEV_SYNC` | `models_dev.sync` | Bool | `true` | models.dev regelmäßig abrufen. Ohne Internetzugang abschalten; dann gilt der Stand, den das Release mitbringt. |
| `KAESO_MODELS_DEV_URL` | `models_dev.url` | Text | `https://models.dev/api.json` | Adresse, von der die Daten von models.dev abgerufen werden. |
| `KAESO_MODELS_DEV_INTERVAL` | `models_dev.interval` | Text | `24h` | Abstand zwischen zwei Abrufen von models.dev. |

### Aufrufe an Anbieter

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_PROVIDERS_OPENAI_BASE_URL` | `providers.openai.base_url` | Text | `https://api.openai.com` | Basis-URL der OpenAI-API. |
| `KAESO_PROVIDERS_ANTHROPIC_BASE_URL` | `providers.anthropic.base_url` | Text | `https://api.anthropic.com` | Basis-URL der Anthropic-API. |
| `KAESO_UPSTREAM_CONNECT_TIMEOUT` | `upstream.connect_timeout` | Text | `10s` | Timeout für den Verbindungsaufbau zu Anbietern, TCP und TLS; höchstens 24h. |
| `KAESO_UPSTREAM_REQUEST_TIMEOUT` | `upstream.request_timeout` | Text | `600s` | Gesamt-Timeout einer Anbieteranfrage ohne Streaming; höchstens 24h. |
| `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` | `upstream.first_byte_timeout` | Text | `60s` | Streaming: Wartezeit bis zum ersten Byte vom Anbieter; höchstens 24h. |
| `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` | `upstream.stream_idle_timeout` | Text | `300s` | Streaming: längste Pause zwischen zwei Chunks; höchstens 24h. |

Timeouts für einzelne Modelle werden am Modell im Katalog eines Projekts festgelegt; siehe [Modelle und Aliasse](../console/models-and-aliases.md).

### Streams an Apps

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_STREAMS_KEEPALIVE_INTERVAL` | `streams.keepalive_interval` | Text | `15s` | Ist eine gestreamte Antwort offen und wurde so lange nichts an die App geschrieben, schreibt das Gateway ein Keepalive, das das SDK der App ignoriert (einen SSE-Kommentar in den OpenAI-Formaten, ein `ping`-Ereignis für Anthropic), damit Proxys mit Leerlauf-Timeout die Verbindung halten. `0s` schaltet Keepalives ab; sonst 1s bis 300s. Siehe [Lange Streams](domains-and-tls.md#lange-streams). |

### Kontinuität des Reasonings

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_CONTINUITY_REASONING_TTL` | `continuity.reasoning_ttl` | Text | `1h` | Wie lange das Gateway das Reasoning vor einem Tool-Aufruf aufbewahrt, wenn eine OpenAI-Chat-Completions-App ein Modell aufruft, das in einem anderen Format angesprochen wird (Chat Completions hat dafür kein Feld), damit die nächste Runde der App es zurückbringt. Liegt in Valkey, mit dem Hauptschlüssel verschlüsselt und nur mit dem API-Schlüssel der Anfrage lesbar; wird nie protokolliert. `0s` schaltet das ab; sonst 1m bis 24h. Siehe [Formatübersetzung](../concepts/format-translation.md). |

### Anfrage- und Antwortkörper

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_BODY_LIMITS_MODEL_API` | `body_limits.model_api` | Text | `32MiB` | Größter Anfragekörper für die Modell-API. |
| `KAESO_BODY_LIMITS_PLATFORM_API` | `body_limits.platform_api` | Text | `1MiB` | Größter Anfragekörper für die Platform API. |
| `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` | `body_limits.provider_response` | Text | `128MiB` | Größter Antwortkörper ohne Streaming, den Kaeso von einem Anbieter liest; bei einem größeren scheitert die Anfrage mit 502. |

### Anfrageprotokolle

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_LOGS_BUFFER_SIZE` | `logs.buffer_size` | Text | `256MiB` | Arbeitsspeicher für Zeilen der Anfrageprotokolle, die auf ClickHouse warten; ist er voll, werden die ältesten Zeilen verworfen. |
| `KAESO_LOGS_FLUSH_INTERVAL` | `logs.flush_interval` | Text | `1s` | Wie oft gepufferte Protokollzeilen nach ClickHouse geschrieben werden. |
| `KAESO_LOGS_FLUSH_ROWS` | `logs.flush_rows` | Ganzzahl | `5000` | Gepufferte Protokollzeilen sofort schreiben, sobald so viele warten; mindestens 1. |

### Standards der Instanz

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_INSTANCE_ORG_CREATION` | `instance.org_creation` | Text | `instance_admins` | Wer auf einer neuen Installation Organisationen erstellen darf: `instance_admins` oder `everyone`. Später ändern Instanz-Admins das in den Instanzeinstellungen und können dort auch Benutzer bestimmter E-Mail-Domains zulassen. |
| `KAESO_INSTANCE_ACCOUNT_CREATION` | `instance.account_creation` | Text | `invitation_only` | Wer auf einer neuen Installation ohne Einladung ein Konto anlegen darf: `invitation_only` oder `everyone`. Später ändern Instanz-Admins das in den Instanzeinstellungen und können dort auch nur bestimmte E-Mail-Domains zulassen. |
| `KAESO_INSTANCE_METADATA_RETENTION_DAYS` | `instance.metadata_retention_days` | Ganzzahl | `90` | Standardaufbewahrung der Metadaten von Anfrageprotokollen für neue Organisationen, in Tagen; mindestens 1, höchstens der Höchstwert unten. |
| `KAESO_INSTANCE_CONTENT_RETENTION_DAYS` | `instance.content_retention_days` | Ganzzahl | `30` | Standardaufbewahrung der Inhalte von Anfrageprotokollen für neue Organisationen, in Tagen; mindestens 1, nie länger als die Aufbewahrung der Metadaten und höchstens der Höchstwert unten. |
| `KAESO_INSTANCE_MAX_METADATA_RETENTION_DAYS` | `instance.max_metadata_retention_days` | Ganzzahl | `3650` | Längste Aufbewahrung der Metadaten, die eine Organisation wählen kann, in Tagen; höchstens 36500. |
| `KAESO_INSTANCE_MAX_CONTENT_RETENTION_DAYS` | `instance.max_content_retention_days` | Ganzzahl | `3650` | Längste Aufbewahrung der Inhalte, die eine Organisation wählen kann, in Tagen; höchstens 36500. |
| `KAESO_INSTANCE_ORG_DELETION` | `instance.org_deletion` | Bool | `true` | Ob sich Organisationen auf einer neuen Installation löschen lassen. Später ändern Instanz-Admins das in den Instanzeinstellungen. |
| `KAESO_INSTANCE_ORG_DELETION_GRACE_DAYS` | `instance.org_deletion_grace_days` | Ganzzahl | `7` | So viele Tage bleibt eine gelöschte Organisation schreibgeschützt und wiederherstellbar, bevor sie endgültig entfernt wird; 1 bis 90. |

### Logging

| Variable | TOML-Schlüssel | Typ | Standard | Was sie bewirkt |
| --- | --- | --- | --- | --- |
| `KAESO_LOG_LEVEL` | `log.level` | Text | `info,sqlx=warn` | Log-Level oder Filter, etwa `info` oder `info,kaeso_server=debug`. |
| `KAESO_LOG_FORMAT` | `log.format` | Text | `json` | `json` für strukturierte Logs, `text` für die Entwicklung. |

## Anbietereinstellungen

Jeder Anbieter hat eigene Einstellungen: den TOML-Schlüssel `providers.<id>.<field>` und die Variable `KAESO_PROVIDERS_<ID>_<FIELD>`, mit der ID in Großbuchstaben und `_` für jedes `-`. Kaeso ruft zwei Anbieter auf, `openai` und `anthropic`, und jeder hat ein Feld, `base_url` (siehe [Aufrufe an Anbieter](#aufrufe-an-anbieter)).

- Anbieter-IDs bestehen aus Kleinbuchstaben und Ziffern mit einzelnen Bindestrichen dazwischen (`^[a-z0-9]+(-[a-z0-9]+)*$`). Dieselbe Regel gilt überall, wo eine ID vorkommt, auch für Anbieter-Schlüssel und Katalogmodelle.
- Eine `KAESO_PROVIDERS_…`-Variable oder eine `[providers.…]`-Tabelle, deren ID die Regel verletzt oder einen Anbieter nennt, den Kaeso nicht aufruft, hält das Gateway beim Start an; die Meldung nennt sie und listet die Anbieter auf.

## Drosselung von API-Schlüsseln

Jeder API-Schlüssel, den das Gateway nicht kürzlich gesehen hat, kostet eine Abfrage in PostgreSQL. Unbekannte Schlüssel merkt es sich 30 Sekunden lang; eine App, die weiter einen alten Schlüssel sendet, kostet daher eine Abfrage pro 30 Sekunden und Gateway-Instanz, ein Client, der für jede Anfrage einen neuen Schlüssel erfindet, aber eine pro Anfrage. Deshalb zählt das Gateway fehlgeschlagene Abfragen pro Client-Adresse in Valkey, über Modell-API und Platform API hinweg. Nach `KAESO_KEY_FAILURES_PER_ADDRESS` Fehlschlägen (Standard 100) innerhalb von `KAESO_KEY_FAILURES_WINDOW` (Standard 1 Minute) ist die Adresse bis zum Ende des Fensters gesperrt: Jeder Schlüssel von ihr, der nicht im Cache liegt, erhält 429 mit `Retry-After` im Format der jeweiligen API, ohne Datenbankabfrage.

- Es zählen nur wohlgeformte Schlüssel, zu denen es keinen Schlüssel gibt. Fehlerhafte Werte, deaktivierte oder abgelaufene Schlüssel und ein unbekannter Schlüssel, der innerhalb von 30 Sekunden wiederkommt, zählen nicht.
- Gültige Schlüssel zählen nie. Ein Schlüssel, der in den letzten 30 Sekunden auf derselben Instanz verwendet wurde, wird aus dem Speicher beantwortet, auch von einer gesperrten Adresse; andere gültige Schlüssel warten das Ende des Fensters ab.
- IPv6-Adressen zählen pro /64.
- Hinter einem Reverse Proxy stammt die Client-Adresse nur dann aus `X-Forwarded-For`, wenn der Proxy in `KAESO_TRUSTED_PROXIES` steht. Sonst sieht jeder Client wie der Proxy aus und teilt dessen Kontingent.
- Solange Valkey nicht erreichbar ist, werden Fehlschläge nicht gezählt.

Jede Sperre erhöht `kaeso_key_failure_blocks_total` und protokolliert `too many failed API key lookups from one address` mit der Adresse; siehe [Zustand und Metriken](monitoring.md).

## Hinweise

- **Standards der Instanz** füllen nur die Instanzeinstellungen einer neuen Installation vor. Danach ändern Instanz-Admins die Erstellung von Konten und Organisationen, das Löschen von Organisationen und dessen Karenzzeit sowie Standards und Höchstwerte der Aufbewahrung unter **Einstellungen** › **Instanz** › **Einstellungen** oder über die Platform API (`instance/settings`); die gespeicherten Werte haben Vorrang.
- **Anmeldemethoden**, die hier ausgeschaltet sind, lassen sich in der Konsole nicht einschalten; sie zeigt sie so, wie die Konfiguration sie festlegt.
- **Entwicklung.** Ist `KAESO_PUBLIC_URL` eine `http`-URL auf einer Loopback-Adresse, etwa `http://localhost:8080`, wird das Sitzungscookie ohne `Secure` und ohne das Präfix `__Host-` gesendet, sodass die Konsole auf dem eigenen Rechner über einfaches HTTP funktioniert. Jede andere `http`-URL wird abgelehnt.
