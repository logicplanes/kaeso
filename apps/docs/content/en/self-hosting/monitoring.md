---
title: Health and metrics
description: The health checks, Prometheus metrics and logs of a running install.
order: 10
---

The gateway reports its health on two endpoints, serves Prometheus metrics on a port of its own, and writes JSON logs. In the console, **Settings** › **Instance** › **System** shows the same health, the running version, the request log buffer and storage.

## Health checks

| Endpoint | Answers |
| --- | --- |
| `GET /healthz` | `{"status": "ok"}` while the process runs |
| `GET /readyz` | `ready`, `degraded` or `not_ready`, with the state of `postgres`, `valkey` and `clickhouse`, each `ok` or `down` |

`/readyz` answers 503 with `not_ready` when PostgreSQL or Valkey can't be reached. When ClickHouse is down or not yet migrated, it answers 200 with `degraded`, because request logs never hold requests up.

Both endpoints are on the gateway's port 8080. Caddy routes only `/v1/` and `/platform/` there, so on your domain these paths reach the console. The gateway's container health check calls `/healthz`, and `docker compose ps` shows the result. To read `/readyz` on the server:

```bash
docker compose exec console wget -qO- http://gateway:8080/readyz
```

## Metrics

The gateway serves Prometheus metrics at `/metrics` on port 9100 (`KAESO_METRICS_LISTEN`). Never route that port publicly. To scrape it from the server, publish it on the loopback address in `compose.override.yaml`:

```yaml title="compose.override.yaml"
services:
  gateway:
    ports:
      - "127.0.0.1:9100:9100"
```

| Metric | Watch for |
| --- | --- |
| `kaeso_requests_total{surface, route, status, provider, model}` | A rise in `status=~"5.."` on `surface="model_api"`. `model` is `custom` for models without models.dev data |
| `kaeso_request_duration_seconds{surface, route}`, `kaeso_time_to_first_byte_seconds{provider}` | Changes in the 99th percentile per route and per provider |
| `kaeso_upstream_errors_total{kind, provider}` | `overloaded`, `timeout` or `connect`: a provider outage. `authentication` or `billing`: a provider key needs attention |
| `kaeso_stream_failures_total{kind, provider}` | Streams that failed after their headers were sent. `kaeso_requests_total` counts them as 200, the request log as failures |
| `kaeso_limit_refusals_total{kind, level}` | Budgets running out |
| `kaeso_reasoning_continuity_total{outcome}` | `unavailable`: Valkey trouble. Many `missing`: conversations outlive `KAESO_CONTINUITY_REASONING_TTL` |
| `kaeso_tokens_total{provider, kind}`, `kaeso_cost_microusd_total{provider}` | Spend trends |
| `kaeso_log_buffer_rows`, `kaeso_log_buffer_bytes` | A growing buffer: ClickHouse isn't accepting writes |
| `kaeso_log_rows_dropped_total`, `kaeso_clickhouse_write_failures_total` | Any increase: log rows were lost or are at risk |
| `kaeso_usage_rows_dropped_total` | Any increase: PostgreSQL refused usage for good, and the log line `a usage row cannot be recorded` says why |
| `kaeso_valkey_failures_total{operation}`, `kaeso_limits_fail_open_total{reason}` | Valkey trouble. Meanwhile, hard budgets refuse requests with 503, and other limits let them through unchecked |
| `kaeso_job_runs_total{job, outcome}` | `outcome="failed"` for background jobs |
| `kaeso_auth_failures_total{method, reason}` | Bursts of failed sign-ins or keys. `reason="throttled"` counts requests refused by [API key throttling](configuration.md#api-key-throttling) |
| `kaeso_key_failure_blocks_total` | Any increase: an address is guessing API keys |
| `kaeso_emails_total{kind, outcome}` | `outcome="failed"` or `dropped`: check the [email](email.md) settings |
| `kaeso_build_info{version}` | The running version |

## Logs

The gateway writes JSON to standard output, one line per event:

```bash
docker compose logs -f gateway
```

Every line a request causes carries its `request_id`, which the reply also sends in the `kaeso-request-id` header. Secrets, provider keys and authorization headers are never logged. `KAESO_LOG_LEVEL` sets the level or a filter, such as `info,kaeso_server=debug`, and `KAESO_LOG_FORMAT=text` writes plain lines for development.
