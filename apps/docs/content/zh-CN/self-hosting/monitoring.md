---
title: 健康状态和指标
description: 运行中实例的健康检查、Prometheus 指标和日志。
order: 10
---

网关通过两个端点报告健康状态，在单独的端口上提供 Prometheus 指标，并输出 JSON 日志。在控制台中，**实例** › **系统**显示同样的健康状态，以及正在运行的版本、请求日志缓冲区和存储占用。

## 健康检查

| 端点 | 响应 |
| --- | --- |
| `GET /healthz` | 进程运行时返回 `{"status": "ok"}` |
| `GET /readyz` | `ready`、`degraded` 或 `not_ready`，并给出 `postgres`、`valkey` 和 `clickhouse` 的状态，每项为 `ok` 或 `down` |

PostgreSQL 或 Valkey 无法访问时，`/readyz` 返回 503 和 `not_ready`。ClickHouse 停止或尚未迁移时，返回 200 和 `degraded`，因为请求日志从不阻塞请求。

两个端点都在网关的端口 8080 上。Caddy 只把 `/v1/`、`/platform/`、`/mcp`、`/oauth/` 和 `/.well-known/oauth-` 转发到这里，所以在您的域名上，这两个路径会到达控制台。网关容器的健康检查调用 `/healthz`，`docker compose ps` 会显示结果。在服务器上读取 `/readyz`：

```bash
docker compose exec console wget -qO- http://gateway:8080/readyz
```

## 指标

网关在端口 9100 的 `/metrics` 上提供 Prometheus 指标（`KAESO_METRICS_LISTEN`）。切勿公开转发该端口。

您自行运行的网关在未设置时监听 `127.0.0.1:9100`。网关镜像在其容器内监听 `0.0.0.0:9100`，而 Compose 不会在任何地方发布该端口。要从服务器上抓取，请在 `compose.override.yaml` 中把它发布到回环地址：

```yaml title="compose.override.yaml"
services:
  gateway:
    ports:
      - "127.0.0.1:9100:9100"
```

| 指标 | 关注点 |
| --- | --- |
| `kaeso_requests_total{surface, route, status, provider, model}` | `surface="model_api"` 上 `status=~"5.."` 的增长。没有 models.dev 数据的模型，`model` 为 `custom` |
| `kaeso_request_duration_seconds{surface, route}`、`kaeso_time_to_first_byte_seconds{provider}` | 各路由和各服务商的 99 分位数变化 |
| `kaeso_gateway_overhead_seconds{provider}` | Kaeso 调用服务商之前自身花费的时间：读取请求体、校验密钥、转换格式和检查限额。分桶从 0.1 ms 开始。如果 `kaeso_time_to_first_byte_seconds` 没有同样上升，多出的耗时来自 Kaeso，而不是服务商 |
| `kaeso_upstream_errors_total{kind, provider}` | `overloaded`、`timeout` 或 `connect`：服务商故障。`authentication` 或 `billing`：某个服务商密钥需要处理 |
| `kaeso_stream_failures_total{kind, provider}` | 在响应头发出后失败的流。`kaeso_requests_total` 把它们计为 200，请求日志则计为失败。`client_stalled`：应用不再读取其流 |
| `kaeso_usage_estimates_total{reason, provider}` | Kaeso 不得不估算的用量。`unreported`：服务商结束回复时没有报告用量。`cancelled` 和 `failed`：断开连接的应用和中途中断的流 |
| `kaeso_limit_refusals_total{kind, level}` | 预算即将用完 |
| `kaeso_reasoning_continuity_total{outcome}` | `unavailable`：Valkey 出现问题。大量 `missing`：对话持续时间超过 `KAESO_CONTINUITY_REASONING_TTL` |
| `kaeso_tokens_total{provider, kind}`、`kaeso_cost_microusd_total{provider}` | 支出趋势 |
| `kaeso_log_buffer_rows`、`kaeso_log_buffer_bytes` | 缓冲区持续增长：ClickHouse 不接受写入 |
| `kaeso_log_rows_dropped_total`、`kaeso_clickhouse_write_failures_total` | 任何增长：日志行已丢失或面临丢失 |
| `kaeso_usage_rows_dropped_total` | 任何增长：PostgreSQL 永久拒绝了用量记录，日志行 `a usage row cannot be recorded` 会说明原因 |
| `kaeso_valkey_failures_total{operation}`、`kaeso_limits_fail_open_total{reason}` | Valkey 出现问题。在此期间，硬性预算以 503 拒绝请求，其他限额则不经检查直接放行 |
| `kaeso_job_runs_total{job, outcome}` | 后台任务的 `outcome="failed"` |
| `kaeso_auth_failures_total{method, reason}` | 登录或密钥失败突增。`reason="throttled"` 统计被 [API 密钥限流](configuration.md#api-密钥限流)拒绝的请求 |
| `kaeso_key_failure_blocks_total` | 任何增长：有地址在猜测 API 密钥 |
| `kaeso_emails_total{kind, outcome}` | `outcome="failed"` 或 `dropped`：检查[邮件](email.md)设置 |
| `kaeso_build_info{version}` | 正在运行的版本 |

## 日志

网关向标准输出写入 JSON，每个事件一行：

```bash
docker compose logs -f gateway
```

请求产生的每一行日志都带有它的 `request_id`，回复也会在 `kaeso-request-id` 响应头中返回该 ID。机密、服务商密钥和 Authorization 请求头从不写入日志。`KAESO_LOG_LEVEL` 设置日志级别或过滤器，例如 `info,kaeso_server=debug`；`KAESO_LOG_FORMAT=text` 为开发输出纯文本行。
