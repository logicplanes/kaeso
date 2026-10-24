---
title: Zustand und Metriken
description: Die Health-Checks, Prometheus-Metriken und Protokolle einer laufenden Installation.
order: 10
---

Das Gateway meldet seinen Zustand an zwei Endpunkten, stellt Prometheus-Metriken auf einem eigenen Port bereit und schreibt JSON-Protokolle. In der Konsole zeigt **Einstellungen** › **Instanz** › **System** denselben Zustand, die laufende Version, den Puffer der Anfrageprotokolle und den Speicher.

## Health-Checks

| Endpunkt | Antwortet |
| --- | --- |
| `GET /healthz` | `{"status": "ok"}`, solange der Prozess läuft |
| `GET /readyz` | `ready`, `degraded` oder `not_ready`, mit dem Zustand von `postgres`, `valkey` und `clickhouse`, jeweils `ok` oder `down` |

`/readyz` antwortet 503 mit `not_ready`, wenn PostgreSQL oder Valkey nicht erreichbar sind. Ist ClickHouse nicht erreichbar oder noch nicht migriert, antwortet es 200 mit `degraded`, denn Anfrageprotokolle halten nie Anfragen auf.

Beide Endpunkte liegen auf Port 8080 des Gateways. Caddy leitet dorthin nur `/v1/` und `/platform/` weiter, unter Ihrer Domain erreichen diese Pfade also die Konsole. Der Health-Check des Gateway-Containers ruft `/healthz` auf, und `docker compose ps` zeigt das Ergebnis. So lesen Sie `/readyz` auf dem Server:

```bash
docker compose exec console wget -qO- http://gateway:8080/readyz
```

## Metriken

Das Gateway stellt Prometheus-Metriken unter `/metrics` auf Port 9100 bereit (`KAESO_METRICS_LISTEN`). Leiten Sie diesen Port nie öffentlich weiter.

Ein Gateway, das Sie selbst starten, lauscht auf `127.0.0.1:9100`, solange Sie nichts anderes setzen. Das Gateway-Image lauscht in seinem Container auf `0.0.0.0:9100`, und Compose veröffentlicht den Port nirgends. Um ihn vom Server aus abzufragen, veröffentlichen Sie ihn in der `compose.override.yaml` auf der Loopback-Adresse:

```yaml title="compose.override.yaml"
services:
  gateway:
    ports:
      - "127.0.0.1:9100:9100"
```

| Metrik | Worauf achten |
| --- | --- |
| `kaeso_requests_total{surface, route, status, provider, model}` | Ein Anstieg von `status=~"5.."` bei `surface="model_api"`. `model` ist `custom` für Modelle ohne Daten von models.dev |
| `kaeso_request_duration_seconds{surface, route}`, `kaeso_time_to_first_byte_seconds{provider}` | Veränderungen im 99. Perzentil je Route und je Anbieter |
| `kaeso_upstream_errors_total{kind, provider}` | `overloaded`, `timeout` oder `connect`: ein Ausfall beim Anbieter. `authentication` oder `billing`: ein Anbieter-Schlüssel muss geprüft werden |
| `kaeso_stream_failures_total{kind, provider}` | Streams, die nach dem Senden ihrer Header scheiterten. `kaeso_requests_total` zählt sie als 200, das Anfrageprotokoll als Fehlschläge |
| `kaeso_usage_estimates_total{reason, provider}` | Nutzung, die Kaeso schätzen musste. `unreported`: ein Anbieter hat eine Antwort ohne Nutzung beendet. `cancelled` und `failed`: Apps, die die Verbindung getrennt haben, und abgebrochene Streams |
| `kaeso_limit_refusals_total{kind, level}` | Budgets, die zur Neige gehen |
| `kaeso_reasoning_continuity_total{outcome}` | `unavailable`: Probleme mit Valkey. Viele `missing`: Unterhaltungen dauern länger als `KAESO_CONTINUITY_REASONING_TTL` |
| `kaeso_tokens_total{provider, kind}`, `kaeso_cost_microusd_total{provider}` | Entwicklung der Ausgaben |
| `kaeso_log_buffer_rows`, `kaeso_log_buffer_bytes` | Ein wachsender Puffer: ClickHouse nimmt keine Schreibvorgänge an |
| `kaeso_log_rows_dropped_total`, `kaeso_clickhouse_write_failures_total` | Jeder Anstieg: Protokollzeilen gingen verloren oder sind gefährdet |
| `kaeso_usage_rows_dropped_total` | Jeder Anstieg: PostgreSQL hat Nutzung endgültig abgelehnt, und die Logzeile `a usage row cannot be recorded` nennt den Grund |
| `kaeso_valkey_failures_total{operation}`, `kaeso_limits_fail_open_total{reason}` | Probleme mit Valkey. Währenddessen lehnen harte Budgets Anfragen mit 503 ab, und andere Limits lassen sie ungeprüft durch |
| `kaeso_job_runs_total{job, outcome}` | `outcome="failed"` bei Hintergrundjobs |
| `kaeso_auth_failures_total{method, reason}` | Häufungen fehlgeschlagener Anmeldungen oder Schlüssel. `reason="throttled"` zählt Anfragen, die die [Drosselung von API-Schlüsseln](configuration.md#drosselung-von-api-schlüsseln) abgelehnt hat |
| `kaeso_key_failure_blocks_total` | Jeder Anstieg: Eine Adresse rät API-Schlüssel |
| `kaeso_emails_total{kind, outcome}` | `outcome="failed"` oder `dropped`: Prüfen Sie die Einstellungen für [E-Mail](email.md) |
| `kaeso_build_info{version}` | Die laufende Version |

## Protokolle

Das Gateway schreibt JSON auf die Standardausgabe, eine Zeile je Ereignis:

```bash
docker compose logs -f gateway
```

Jede Zeile, die eine Anfrage auslöst, trägt ihre `request_id`, die die Antwort auch im Header `kaeso-request-id` sendet. Secrets, Anbieter-Schlüssel und Authorization-Header werden nie protokolliert. `KAESO_LOG_LEVEL` setzt das Level oder einen Filter, etwa `info,kaeso_server=debug`, und `KAESO_LOG_FORMAT=text` schreibt für die Entwicklung einfache Zeilen.
