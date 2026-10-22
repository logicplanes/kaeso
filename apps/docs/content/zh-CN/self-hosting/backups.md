---
title: 备份和主密钥
description: 备份数据库和主密钥、从备份恢复，以及轮换主密钥。
nav: 备份
order: 9
---

需要备份三样东西，并把主密钥与数据库备份分开存放：

| 内容 | 保存的数据 | 没有备份时 |
| --- | --- | --- |
| PostgreSQL | 用户、组织、项目、密钥、加密的服务商密钥、目录、限额、用量和审计日志 | 整个实例都将丢失 |
| ClickHouse | 请求日志和用量汇总 | 日志历史丢失；限额和预算不依赖它 |
| 主密钥 | 用于加密已保存服务商密钥的密钥 | 必须重新输入每个服务商密钥 |

Valkey 只保存计数器、缓存和短期数据，无需备份。网关会从 PostgreSQL 重建缺失的预算计数器，速率窗口则重新开始。

## 备份 PostgreSQL

在 Kaeso 运行时转储数据库：

```bash
docker compose exec -T postgres pg_dump -U kaeso -Fc kaeso > kaeso-$(date +%F).dump
```

## 恢复 PostgreSQL

停止网关，恢复转储，再启动网关。`--clean --if-exists` 会替换数据库中的现有内容，因此同样的命令既适用于现有实例，也适用于空数据库：

```bash
docker compose stop gateway
docker compose exec -T postgres pg_restore -U kaeso -d kaeso --clean --if-exists < kaeso-2026-10-15.dump
docker compose start gateway
```

在新服务器上恢复：

1. 把 `compose.yaml`、`.env` 和 `master-keys` 放回实例文件夹，并像[创建主密钥](../get-started/quick-start.md#创建主密钥)中那样把 `master-keys` 交给网关的用户。
2. 用 `docker compose up -d --wait postgres` 只启动 PostgreSQL。
3. 用上面的 `pg_restore` 命令恢复转储。
4. 用 `docker compose up -d --wait` 启动其余服务。

## 备份 ClickHouse

使用 ClickHouse 的 `BACKUP` 命令备份到为备份配置的磁盘，或使用 `clickhouse-backup` 之类的工具。小型实例也可以停止 ClickHouse 并复制它的卷：

```bash
docker compose stop clickhouse
```

用常用的卷备份方式复制 `clickhouse` 卷，然后运行 `docker compose start clickhouse`。ClickHouse 停止期间，请求照常工作，其日志行在网关内存中等待，最多到 `KAESO_LOGS_BUFFER_SIZE`。

## 妥善保管主密钥

主密钥就是 `compose.yaml` 旁边的 `master-keys` 文件。已保存的服务商密钥只记录加密它的主密钥的 ID，因此没有对应主密钥，数据库备份就无法解密服务商密钥。请把一份副本保存在密码管理器或机密存储中，不要放在数据库转储旁边。

## 轮换主密钥

`master-keys` 包含一个或多个以逗号分隔的密钥，当前使用的密钥排在最前：`k7c21e9d04a:…,k3f9a01bc2d:…`。Kaeso 用第一个密钥加密新机密，旧机密仍用各自的密钥解密。

1. 生成一个新密钥，放在旧密钥前面：

```bash
umask 077
printf '%s,%s\n' "$(docker compose exec -T gateway kaeso master-key generate)" "$(sudo cat master-keys)" > master-keys.new
sudo chown 65532:65532 master-keys.new && sudo chmod 0400 master-keys.new
mv master-keys.new master-keys
```

2. 重新创建网关，让它读取新文件：

```bash
docker compose up -d --wait --force-recreate gateway
```

3. 用新密钥重新加密每个已保存的机密：

```bash
docker compose exec gateway kaeso master-key rewrap
```

该命令会报告每个密钥 ID 仍在加密多少个机密，以及哪些旧 ID 已不再被使用。

4. 从 `master-keys` 中删除不再使用的旧密钥，并用第 2 步的命令重新创建网关。有多个网关实例时，请等每个实例都使用新列表运行后再删除。

> [!WARNING]
> 只要已保存的机密使用了未配置的密钥 ID，网关就不会启动。在 `rewrap` 报告某个密钥未被使用之前，切勿删除它。

## 后续步骤

- [升级](upgrades.md)：更新到新版本，并留有可以回退的备份。
- [健康状态和指标](monitoring.md)：持续关注实例是否出现问题。
