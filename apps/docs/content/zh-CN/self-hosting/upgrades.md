---
title: 升级
description: 将安装迁移到新版本。
order: 6
---

版本遵循语义化版本规范，Kaeso 的所有组成部分都使用该版本的版本号。每个版本的发布说明位于[发布页面](https://github.com/logicplanes/kaeso/releases)，其 `compose.yaml` 固定了该版本的网关和控制台镜像。

## 升级

1. 阅读发布说明，看看是否有需要处理的事项。
2. [备份](backups.md) PostgreSQL。
3. 下载新版本的 `compose.yaml`，覆盖旧文件。`.env`、`master-keys` 和 `compose.override.yaml` 中的设置保持不变。

```bash
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

4. 启动新版本：

```bash
docker compose pull
docker compose up -d --wait
```

`--wait` 会等新的网关和控制台报告 healthy 后才返回，有一个达不到时命令失败，因此执行升级的脚本或人员会立即发现启动失败。

网关在启动时于锁的保护下迁移 PostgreSQL 和 ClickHouse，因此多个实例可以同时启动，只有一个执行迁移。迁移只向前进行且按顺序运行，因此升级时可以跳过版本。Kaeso 0.1.0 通过一组初始迁移创建完整的数据库结构；之后的版本在其后添加各自的迁移。

## 保持不变的名称

以下名称在每个版本中都相同，因此下载新的 `compose.yaml` 并运行 `docker compose pull` 后数据依然保留：

- 下载地址 `https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml`；
- 镜像 `ghcr.io/logicplanes/kaeso-gateway` 和 `ghcr.io/logicplanes/kaeso-console`，标签为该版本的版本号；
- Compose 项目 `kaeso` 及其服务 `caddy`、`gateway`、`console`、`postgres`、`clickhouse` 和 `valkey`；
- 存放数据的卷 `kaeso_postgres`、`kaeso_clickhouse` 和 `kaeso_valkey`，以及存放 TLS 证书的卷 `kaeso_caddy_data` 和 `kaeso_caddy_config`。

请在安装所在的文件夹中运行 Compose，不要使用 `-p` 或 `COMPOSE_PROJECT_NAME`，这样它会使用项目名 `kaeso`：换成其他项目名时，Compose 会从新的空卷开始。

## 降级

迁移无法撤销。PostgreSQL 数据库若已被更新的版本迁移过，网关会拒绝启动并说明原因。ClickHouse 数据库若已被更新的版本迁移过，网关仍会启动，但不写入请求日志，并记录 `ClickHouse migrations failed; request logs are not written`。

要回到旧版本，先恢复升级前的 PostgreSQL 备份；如果新版本迁移过 ClickHouse，再恢复 [ClickHouse 备份](backups.md#clickhouse)，然后启动旧版本的 `compose.yaml`。
