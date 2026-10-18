---
title: Zustand und Metriken
description: Health-Checks, Prometheus-Metriken und Logs einer laufenden Installation.
order: 9
---

## Health-Checks

| Endpunkt | Antwort |
| --- | --- |
| `GET /healthz` | `{"status": "ok"}`, solange der Prozess läuft |
| `GET /readyz` | `ready`, `degraded` oder `not_ready`, mit dem Zustand von `postgres`, `valkey` und `clickhouse`, jeweils `ok` oder `down` |

`/readyz` antwortet mit 503 und `not_ready`, wenn PostgreSQL oder Valkey nicht erreichbar sind. Ist ClickHouse ausgefallen oder noch nicht migriert, antwortet es mit 200 und `degraded`, denn Anfrageprotokolle halten nie eine Anfrage auf.

Beide Endpunkte liegen auf Port 8080 des Gateways, den der mitgelieferte Caddy nur für `/v1/` und `/platform/` weiterleitet; unter der eigenen Domain landen diese Pfade bei der Konsole. Der Health-Check des Gateway-Containers ruft `/healthz` auf, und `docker compose ps` zeigt sein Ergebnis. Um `/readyz` auf dem Server abzufragen:

```bash
docker compose exec console wget -qO- http://gateway:8080/readyz
```

In der Konsole zeigt **Einstellungen** › **Instanz** › **System** dieselben Zustände, dazu die laufende Version, den Puffer der Anfrageprotokolle und den Speicher.

## Metriken

Das Gateway stellt Prometheus-Metriken unter `/metrics` auf Port 9100 bereit (`KAESO_METRICS_LISTEN`). Diesen Port nie öffentlich weiterleiten. Um ihn vom Host aus abzufragen, ihn in der `compose.override.yaml` auf der Loopback-Adresse veröffentlichen:

```yaml
services:
  gateway:
    ports:
      - "127.0.0.1:9100:9100"
```

| Metrik | Worauf achten |
| --- | --- |
| `kaeso_requests_total{surface, route, status, provider, model}` | Ein Anstieg von `status=~"5.."` bei `surface="model_api"`; `model` ist `custom` für Modelle, die nicht in den Daten von models.dev stehen |
| `kaeso_request_duration_seconds{surface, route}`, `kaeso_time_to_first_byte_seconds{provider}` | Veränderungen im 99. Perzentil pro Route und pro Anbieter |
| `kaeso_upstream_errors_total{kind, provider}` | `overloaded`, `timeout`, `connect`: ein Ausfall beim Anbieter; `authentication`, `billing`: ein Anbieter-Schlüssel braucht Aufmerksamkeit |
| `kaeso_stream_failures_total{kind, provider}` | Gestreamte Antworten, die nach dem Senden ihrer Header fehlschlugen (`stream_interrupted`, `timeout`, `overloaded`, …); `kaeso_requests_total` zählt sie als 200, das Anfrageprotokoll als Fehlschläge |
| `kaeso_limit_refusals_total{kind, level}` | Budgets, die zur Neige gehen |
| `kaeso_reasoning_continuity_total{outcome}` | `unavailable`: Probleme mit Valkey, sodass Chat-Completions-Apps das Reasoning eines Modells zwischen Tool-Aufrufen verlieren; viele `missing`: Gespräche dauern länger als `KAESO_CONTINUITY_REASONING_TTL` |
| `kaeso_tokens_total{provider, kind}`, `kaeso_cost_microusd_total{provider}` | Entwicklung der Ausgaben |
| `kaeso_log_buffer_rows`, `kaeso_log_buffer_bytes` | Ein wachsender Puffer: ClickHouse nimmt keine Schreibvorgänge an |
| `kaeso_log_rows_dropped_total`, `kaeso_clickhouse_write_failures_total` | Jeder Anstieg: Protokollzeilen gingen verloren oder sind gefährdet |
| `kaeso_usage_rows_dropped_total` | Jeder Anstieg: PostgreSQL hat die Tagesnutzung eines Schlüssels und Modells endgültig abgelehnt, sodass sie in Nutzungsberichten fehlt; die Logzeile `a usage row cannot be recorded` nennt den Grund |
| `kaeso_valkey_failures_total{operation}`, `kaeso_limits_fail_open_total{reason}` | Probleme mit Valkey: Solange sie andauern, lehnen harte Budgets Anfragen ab (503), und andere Limits lassen sie ungeprüft durch, was `kaeso_limits_fail_open_total` zählt |
| `kaeso_job_runs_total{job, outcome}` | `outcome="failed"` bei Hintergrundjobs |
| `kaeso_auth_failures_total{method, reason}` | Häufungen fehlgeschlagener Anmeldungen oder Schlüssel; `reason="throttled"`: Anfragen, die abgelehnt wurden, weil ihre Adresse zu viele fehlgeschlagene Schlüsselabfragen hatte |
| `kaeso_key_failure_blocks_total` | Jeder Anstieg: Eine Adresse rät API-Schlüssel; die Logzeile `too many failed API key lookups from one address` nennt sie |
| `kaeso_emails_total{kind, outcome}` | `outcome="failed"` oder `dropped`: die [E-Mail](email.md)-Einstellungen prüfen |
| `kaeso_build_info{version}` | Die laufende Version |

## Logs

Das Gateway schreibt JSON auf die Standardausgabe, eine Zeile pro Ereignis:

```bash
docker compose logs -f gateway
```

Jede Zeile, die eine Anfrage auslöst, trägt deren `request_id`, die auch die Antwort im Header `kaeso-request-id` sendet. Secrets, Anbieter-Schlüssel und Authorization-Header werden nie protokolliert. `KAESO_LOG_LEVEL` legt die Stufe oder einen Filter fest, etwa `info,kaeso_server=debug`; `KAESO_LOG_FORMAT=text` schreibt einfache Zeilen für die Entwicklung.
