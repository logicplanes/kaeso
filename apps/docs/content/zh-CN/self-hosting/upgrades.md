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
docker compose up -d
```

网关在启动时于锁的保护下迁移 PostgreSQL 和 ClickHouse，因此多个实例可以同时启动，只有一个执行迁移。迁移只向前进行且按顺序运行，因此升级时可以跳过版本。

## 降级

如果数据库已被更新的版本迁移过，网关会拒绝启动并说明这一点。要回到旧版本，先恢复升级前所做的备份，再启动旧版本的 `compose.yaml`。
