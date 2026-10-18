---
title: 健康状态和指标
description: 运行中安装的健康检查、Prometheus 指标和日志。
order: 10
---

## 健康检查

| 端点 | 响应 |
| --- | --- |
| `GET /healthz` | 进程运行期间返回 `{"status": "ok"}` |
| `GET /readyz` | `ready`、`degraded` 或 `not_ready`，并附带 `postgres`、`valkey` 和 `clickhouse` 的状态，各为 `ok` 或 `down` |

无法连接 PostgreSQL 或 Valkey 时，`/readyz` 返回 503 和 `not_ready`。ClickHouse 宕机或尚未完成迁移时，它返回 200 和 `degraded`，因为请求日志从不阻塞请求。

两个端点都在网关的 8080 端口上，而内置的 Caddy 只为 `/v1/` 和 `/platform/` 转发该端口；在自己的域名上，这两个路径会到达控制台。网关容器的健康检查调用 `/healthz`，`docker compose ps` 会显示其结果。在服务器上读取 `/readyz`：

```bash
docker compose exec console wget -qO- http://gateway:8080/readyz
```

在控制台中，**设置** › **实例** › **系统**显示相同的状态，以及运行中的版本、请求日志缓冲区和存储。

## 指标

网关在 9100 端口的 `/metrics` 上提供 Prometheus 指标（`KAESO_METRICS_LISTEN`）。永远不要公开转发该端口。要从主机上抓取，请在 `compose.override.yaml` 中把它发布到回环地址：

```yaml
services:
  gateway:
    ports:
      - "127.0.0.1:9100:9100"
```

| 指标 | 关注点 |
| --- | --- |
| `kaeso_requests_total{surface, route, status, provider, model}` | `surface="model_api"` 上 `status=~"5.."` 的上升；不在 models.dev 数据中的模型，其 `model` 为 `custom` |
| `kaeso_request_duration_seconds{surface, route}`、`kaeso_time_to_first_byte_seconds{provider}` | 各路由和各服务商第 99 百分位的变化 |
| `kaeso_upstream_errors_total{kind, provider}` | `overloaded`、`timeout`、`connect`：服务商故障；`authentication`、`billing`：某个服务商密钥需要处理 |
| `kaeso_stream_failures_total{kind, provider}` | 发送响应头之后失败的流式回复（`stream_interrupted`、`timeout`、`overloaded` 等）；`kaeso_requests_total` 将其计为 200，请求日志将其计为失败 |
| `kaeso_limit_refusals_total{kind, level}` | 预算即将耗尽 |
| `kaeso_reasoning_continuity_total{outcome}` | `unavailable`：Valkey 出现问题，Chat Completions 应用在工具调用之间丢失模型的推理；大量 `missing`：对话持续时间超过 `KAESO_CONTINUITY_REASONING_TTL` |
| `kaeso_tokens_total{provider, kind}`、`kaeso_cost_microusd_total{provider}` | 支出趋势 |
| `kaeso_log_buffer_rows`、`kaeso_log_buffer_bytes` | 缓冲区持续增长：ClickHouse 不接受写入 |
| `kaeso_log_rows_dropped_total`、`kaeso_clickhouse_write_failures_total` | 任何增长：日志行已丢失或面临丢失风险 |
| `kaeso_usage_rows_dropped_total` | 任何增长：PostgreSQL 永久拒绝了某个密钥和模型一天的用量，用量报告中会缺少这部分；日志行 `a usage row cannot be recorded` 会说明原因 |
| `kaeso_valkey_failures_total{operation}`、`kaeso_limits_fail_open_total{reason}` | Valkey 出现问题：问题持续期间，硬预算拒绝请求（503），其他限额不经检查直接放行，`kaeso_limits_fail_open_total` 统计后者 |
| `kaeso_job_runs_total{job, outcome}` | 后台任务的 `outcome="failed"` |
| `kaeso_auth_failures_total{method, reason}` | 登录或密钥失败突然增多；`reason="throttled"`：因所在地址查找密钥失败次数过多而被拒绝的请求 |
| `kaeso_key_failure_blocks_total` | 任何增长：有地址在猜测 API 密钥；日志行 `too many failed API key lookups from one address` 会指明该地址 |
| `kaeso_emails_total{kind, outcome}` | `outcome="failed"` 或 `dropped`：检查[电子邮件](email.md)设置 |
| `kaeso_build_info{version}` | 运行中的版本 |

## 日志

网关向标准输出写入 JSON，每个事件一行：

```bash
docker compose logs -f gateway
```

一个请求引起的每一行日志都带有其 `request_id`，回复也会在 `kaeso-request-id` 响应头中发送该 ID。机密、服务商密钥和 Authorization 请求头从不记录到日志。`KAESO_LOG_LEVEL` 设置级别或过滤器，例如 `info,kaeso_server=debug`；`KAESO_LOG_FORMAT=text` 输出适合开发使用的纯文本行。
