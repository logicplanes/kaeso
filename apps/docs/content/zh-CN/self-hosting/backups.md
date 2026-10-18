---
title: 备份和主密钥
description: 需要备份什么、如何恢复，以及如何轮换主密钥。
order: 9
---

需要备份三样东西，并将主密钥与数据库备份分开存放。

| 内容 | 保存的数据 | 没有备份时 |
| --- | --- | --- |
| PostgreSQL | 用户、组织、项目、密钥、加密的服务商密钥、目录、限额、用量记录、审计日志 | 整个安装都将丢失 |
| ClickHouse | 请求日志和用量汇总 | 日志历史丢失；限额和预算不依赖它 |
| 主密钥 | 用于加密已保存服务商密钥的密钥 | 必须重新输入所有服务商密钥 |

Valkey 只保存计数器、缓存和短期数据，无需备份：预算计数器缺失时会从 PostgreSQL 重建，速率窗口则重新开始。

## PostgreSQL

在 Kaeso 运行时转储数据库：

```bash
docker compose exec -T postgres pg_dump -U kaeso -Fc kaeso > kaeso-$(date +%F).dump
```

在网关停止时恢复。`--clean --if-exists` 会替换数据库中的现有内容，因此同一条命令既适用于现有安装，也适用于空数据库：

```bash
docker compose stop gateway
docker compose exec -T postgres pg_restore -U kaeso -d kaeso --clean --if-exists < kaeso-2026-10-15.dump
docker compose start gateway
```

在新服务器上，先放回 `compose.yaml`、`.env` 和 `master-keys`，用 `docker compose up -d --wait postgres` 只启动 PostgreSQL，完成恢复后，再用 `docker compose up -d --wait` 启动其余服务。

## ClickHouse

使用 ClickHouse 的 `BACKUP` 命令备份到为备份配置的磁盘，或使用 `clickhouse-backup` 之类的工具。对小型安装，停止 ClickHouse 并复制其卷也可以：

```bash
docker compose stop clickhouse
```

用常用的卷备份方式复制 `clickhouse` 卷，然后运行 `docker compose start clickhouse`。ClickHouse 停止期间，请求照常工作，请求日志行在网关内存中等待，最多到 `KAESO_LOGS_BUFFER_SIZE`。

## 主密钥

主密钥就是 `compose.yaml` 旁边的 `master-keys` 文件，网关以 Docker secret `master_keys` 的形式读取它。已保存的服务商密钥只记录加密它的主密钥的 ID，从不记录主密钥本身，因此没有对应主密钥的数据库备份无法解密服务商密钥。请把一份副本保存在密码管理器或机密存储中，而不是放在数据库转储旁边。

## 轮换主密钥

`master-keys` 包含一个或多个以逗号分隔的密钥，当前使用的密钥排在最前：`k7c21e9d04a:…,k3f9a01bc2d:…`。新机密用第一个密钥加密；旧机密仍用各自的密钥解密。

1. 生成一个新密钥，并放在旧密钥前面：

```bash
printf '%s,%s\n' "$(docker compose exec -T gateway kaeso master-key generate)" "$(cat master-keys)" > master-keys.new
mv master-keys.new master-keys
chmod 644 master-keys
```

2. 重新创建网关，让它读取新文件：

```bash
docker compose up -d --wait --force-recreate gateway
```

3. 用新密钥重新加密每个已保存的机密：

```bash
docker compose exec gateway kaeso master-key rewrap
```

它会报告每个密钥 ID 仍在加密多少个机密，以及哪些旧 ID 已不再被使用。

4. 旧 ID 不再被使用后，将其从 `master-keys` 中删除，并用第 2 步的命令重新创建网关。有多个网关实例时，等每个实例都使用新列表运行后再删除。

> [!WARNING]
> 如果已保存的机密使用了未配置的密钥 ID，网关会拒绝启动并指明该 ID。在 `rewrap` 报告某个密钥未被使用之前，切勿删除它。
