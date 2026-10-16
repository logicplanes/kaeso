---
title: 健康状态和指标
description: 运行中安装的健康检查、Prometheus 指标和日志。
order: 8
---

## 健康检查

| 端点 | 响应 |
| --- | --- |
| `GET /healthz` | 进程运行期间返回 `{"status": "ok"}` |
| `GET /readyz` | `ready`、`degraded` 或 `not_ready`，并附上 PostgreSQL、Valkey 和 ClickHouse 的状态，每项为 `ok` 或 `down` |

无法连接 PostgreSQL 或 Valkey 时，`/readyz` 以 503 返回 `not_ready`。ClickHouse 停止或尚未迁移时，它返回 `degraded`，但仍处于就绪状态，因为写入日志从不会阻塞请求。

这两个端点都位于网关的 8080 端口。内置的 Caddy 只将 `/v1/` 和 `/platform/` 路由到网关，因此从外部无法访问它们；网关镜像自带的健康检查使用 `/healthz`，`docker compose ps` 会显示其结果。在控制台中，**设置** › **实例** › **系统**会显示相同的状态、正在运行的版本、请求日志缓冲区和存储情况。

## 指标

网关在 9100 端口（`KAESO_METRICS_LISTEN`）的 `/metrics` 上提供 Prometheus 指标。切勿将该端口公开路由。要从主机上抓取指标，在 `compose.override.yaml` 中将其发布到回环地址：

```yaml
services:
  gateway:
    ports:
      - "127.0.0.1:9100:9100"
```

| 指标 | 关注 |
| --- | --- |
| `kaeso_requests_total{surface, route, status, provider, model}` | `surface="model_api"` 上 `status=~"5.."` 的增加 |
| `kaeso_request_duration_seconds`、`kaeso_time_to_first_byte_seconds` | 各服务商 99 分位数的变化 |
| `kaeso_upstream_errors_total{kind, provider}` | 服务商故障：`overloaded`、`timeout`、`connect` |
| `kaeso_stream_failures_total{kind, provider}` | 在响应头发出之后失败的流式回复（`stream_interrupted`、`timeout`、`overloaded` 等）；`kaeso_requests_total` 将其计为 200，请求日志则将其计为失败 |
| `kaeso_limit_refusals_total{kind, level}` | 预算耗尽 |
| `kaeso_reasoning_continuity_total{outcome}` | `unavailable`：Valkey 出现问题，Chat Completions 应用会在工具调用之间丢失模型的推理；大量 `missing`：对话持续时间超过了 `KAESO_CONTINUITY_REASONING_TTL` |
| `kaeso_tokens_total{provider, kind}`、`kaeso_cost_microusd_total{provider}` | 支出趋势 |
| `kaeso_log_buffer_rows`、`kaeso_log_buffer_bytes` | 缓冲区持续增长：ClickHouse 未接受写入 |
| `kaeso_log_rows_dropped_total`、`kaeso_clickhouse_write_failures_total` | 任何增加：日志行已丢失或面临丢失风险 |
| `kaeso_valkey_failures_total{operation}`、`kaeso_limits_fail_open_total` | Valkey 出现问题；在此期间硬预算会拒绝请求（503） |
| `kaeso_job_runs_total{job, outcome}` | 后台任务的 `outcome="failed"` |
| `kaeso_auth_failures_total{method, reason}` | 登录或密钥失败突然增多；`reason="throttled"`：因所在地址密钥查找失败次数过多而被拒绝的请求 |
| `kaeso_key_failure_blocks_total` | 任何增加：某个地址正在猜测 API 密钥；日志行 `too many failed API key lookups from one address` 会指明该地址 |
| `kaeso_emails_total{kind, outcome}` | `outcome="failed"` 或 `dropped`：检查[电子邮件](email.md)设置 |
| `kaeso_build_info` | 正在运行的版本 |

## 日志

网关将 JSON 写入标准输出，每个事件一行，请求引发的所有内容都带有请求 ID（`kaeso-request-id`）。机密、服务商密钥和授权请求头从不记录到日志。

```bash
docker compose logs -f gateway
```

`KAESO_LOG_LEVEL` 设置日志级别或过滤器，例如 `info,kaeso_server=debug`。`KAESO_LOG_FORMAT=text` 输出普通文本行，用于开发。
