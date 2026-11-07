---
title: 多个网关
description: 在 Caddy 或负载均衡器后面运行多个网关，并了解它们共享什么。
order: 13
---

一个网关能处理数千个连接。当一个不够用，或者需要在一个重启时另一个继续服务，就运行多个。它们共享请求所依赖的一切，因此任何网关都能应答任何请求。

## 网关共享的内容

一个安装中的每个网关都读取同一套 PostgreSQL、Valkey 和 ClickHouse，不会把请求需要的任何东西留在自己这里：

| 共享方式 | 内容 |
| --- | --- |
| PostgreSQL | 用户、会话、组织、项目、密钥、目录、限额、用量和审计日志；在一个网关上登录的会话在其他网关上同样有效 |
| Valkey | 速率、token 和预算计数器，因此限额对所有网关生效；[响应缓存](../console/response-cache.md)；推理连续性；密钥、目录和设置的共享缓存，失效通知在一秒内到达每个网关；哪些[别名路由](../concepts/alias-routes.md#被跳过的步骤)步骤刚刚失败，一个网关失败的调用因此替其他网关省下 |
| PostgreSQL 锁 | 后台任务：KPMR 同步、更新检查、通知和清理每次只在一个网关上运行，而不是每个网关各运行一次 |

每个网关只保留属于自己进程的内容：每秒写入的请求日志行、每 10 秒记录的用量、用于翻译流的内存预算，以及自己的指标。请抓取每个网关的 `/metrics`；计数器可以相加（[健康状态和指标](monitoring.md#指标)）。

## 在 Compose 软件包中

在 `.env` 中设置网关数量并应用：

```bash
echo GATEWAY_REPLICAS=2 >> .env
docker compose up -d --wait
```

Compose 会启动 `gateway-1`、`gateway-2` 等，每个都有自己的内存和进程数上限（[容器限制](configuration.md#容器限制)）。Caddy 每秒通过 Docker 的 DNS 解析 `gateway`，把每个请求发给打开连接最少的网关，并在某个网关失败后 5 秒内跳过它。`docker compose exec gateway` 连接到第一个网关，对 `kaeso setup-link` 和其他命令来说足够了。

更新器会处理每个副本：先停止全部，再逐个启动新的并等待各自健康，这样不会有旧网关在新网关迁移数据库时继续运行（[更新器](upgrades.md#更新器)）。

## 带来的好处

2026 年 11 月 6 日在一台 Apple M 系列 Mac 上用 Docker 29.5 测量：每个网关限制为一个 CPU，前面是 Caddy，后面是模拟服务商，64 个连接持续 20 秒发送非流式聊天补全（三次运行取中间值）：

| 网关数 | 每秒请求数 | 中位数 | 第 99 百分位 |
| --- | --- | --- | --- |
| 1 | 8,670 | 7.2 ms | 12.2 ms |
| 2 | 14,836 | 4.1 ms | 8.9 ms |
| 3 | 17,361 | 3.4 ms | 8.6 ms |

两个网关处理的请求是一个的 1.7 倍，延迟减半；第三个增益较小，因为 Caddy、模拟服务商和负载生成器共用同一台机器。在不同主机上，或者每个网关各有一个 CPU 时，增益接近网关数量。流等待的是服务商而不是 CPU，因此获得的是容量而非速度：每个网关有自己的 `KAESO_SERVER_MAX_CONNECTIONS`（[配置](configuration.md#设置)）。

一个网关因更新或崩溃而重启时，其他网关照常服务，没有代价。Caddy 会在 5 秒内停止向它发送请求，并在它恢复健康后重新启用。

## 在托管平台上

[平台模板](platforms/index.md)在 AWS 上以 `GatewayCount`、在 Google Cloud 上以 `gateway_min_instances`、在 DigitalOcean 上以 `instance_count` 接受网关数量；Render 和 Railway 在各自的控制面板中扩展服务。前面的负载均衡器不需要任何会话粘性，因为请求依赖的内容都不保存在单个网关上。

## 下一步

- [配置](configuration.md)：`GATEWAY_REPLICAS` 和每个网关获得的限制。
- [健康状态和指标](monitoring.md)：抓取每个网关并读取可相加的计数器。
- [升级](upgrades.md)：更新器如何替换每个副本。
