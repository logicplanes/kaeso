---
title: Health and metrics
description: Health checks, Prometheus metrics and logs of a running install.
order: 8
---

## Health checks

| Endpoint | Answer |
| --- | --- |
| `GET /healthz` | `{"status": "ok"}` while the process runs |
| `GET /readyz` | `ready`, `degraded` or `not_ready`, with the state of PostgreSQL, Valkey and ClickHouse, each `ok` or `down` |

`/readyz` answers 503 with `not_ready` when PostgreSQL or Valkey can't be reached. ClickHouse being down, or not yet migrated, makes it `degraded` but still ready, because writing logs never holds requests up.

Both endpoints are on the gateway's port 8080. The bundled Caddy routes only `/v1/` and `/platform/` to the gateway, so on your domain these paths reach the console instead. The gateway image's own health check uses `/healthz`, and `docker compose ps` shows the result. To read `/readyz` from the server:

```bash
docker compose exec console wget -qO- http://gateway:8080/readyz
```

In the console, **Settings** › **Instance** › **System** shows the same states, the running version, the request log buffer and storage.

## Metrics

The gateway serves Prometheus metrics at `/metrics` on port 9100 (`KAESO_METRICS_LISTEN`). Never route that port publicly. To scrape it from the host, publish it on the loopback address in `compose.override.yaml`:

```yaml
services:
  gateway:
    ports:
      - "127.0.0.1:9100:9100"
```

| Metric | Watch for |
| --- | --- |
| `kaeso_requests_total{surface, route, status, provider, model}` | A rise in `status=~"5.."` on `surface="model_api"`; `model` is `custom` for models that aren't in the models.dev data |
| `kaeso_request_duration_seconds{surface, route}`, `kaeso_time_to_first_byte_seconds{provider}` | Changes in the 99th percentile per route and per provider |
| `kaeso_upstream_errors_total{kind, provider}` | Provider outages: `overloaded`, `timeout`, `connect`; `authentication` or `billing`: a provider key needs attention |
| `kaeso_stream_failures_total{kind, provider}` | Streamed replies that failed after their headers were sent (`stream_interrupted`, `timeout`, `overloaded`, …); `kaeso_requests_total` counts them as 200, the request log as failures |
| `kaeso_limit_refusals_total{kind, level}` | Budgets running out |
| `kaeso_reasoning_continuity_total{outcome}` | `unavailable`: Valkey trouble, and Chat Completions apps lose a model's reasoning between tool calls; many `missing`: conversations outlive `KAESO_CONTINUITY_REASONING_TTL` |
| `kaeso_tokens_total{provider, kind}`, `kaeso_cost_microusd_total{provider}` | Spend trends |
| `kaeso_log_buffer_rows`, `kaeso_log_buffer_bytes` | A growing buffer: ClickHouse isn't accepting writes |
| `kaeso_log_rows_dropped_total`, `kaeso_clickhouse_write_failures_total` | Any increase: log rows were lost or are at risk |
| `kaeso_usage_rows_dropped_total` | Any increase: PostgreSQL refused a day's usage of a key and model for good, so usage reports miss it; the log line `a usage row cannot be recorded` says why |
| `kaeso_valkey_failures_total{operation}`, `kaeso_limits_fail_open_total{reason}` | Valkey trouble: while it lasts, hard budgets refuse requests (503), and other limits let them through unchecked, which `kaeso_limits_fail_open_total` counts |
| `kaeso_job_runs_total{job, outcome}` | `outcome="failed"` for background jobs |
| `kaeso_auth_failures_total{method, reason}` | Bursts of failed sign-ins or keys; `reason="throttled"`: requests refused because their address made too many failed key lookups |
| `kaeso_key_failure_blocks_total` | Any increase: an address is guessing API keys; the log line `too many failed API key lookups from one address` names it |
| `kaeso_emails_total{kind, outcome}` | `outcome="failed"` or `dropped`: check the [email](email.md) settings |
| `kaeso_build_info` | The running version |

## Logs

The gateway writes JSON to standard output, one line per event. Everything a request causes carries its `request_id`, the id its reply sends in the `kaeso-request-id` header. Secrets, provider keys and authorization headers are never logged.

```bash
docker compose logs -f gateway
```

`KAESO_LOG_LEVEL` sets the level or a filter, such as `info,kaeso_server=debug`. `KAESO_LOG_FORMAT=text` writes plain lines, for development.
