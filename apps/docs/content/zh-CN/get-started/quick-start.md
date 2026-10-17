---
title: 快速开始
description: 用 Docker Compose 安装 Kaeso，连接服务商并发送第一个请求。
order: 1
---

一个 Compose 文件即可运行整个安装：网关、控制台、PostgreSQL、ClickHouse、Valkey，以及为域名自动获取 TLS 证书的 Caddy。本页从一台空服务器开始，一直到发出第一个请求。

## 准备工作

- 一台装有 Docker Engine 和 Docker Compose 2.24 或更高版本的服务器，2 个 vCPU、4 GB 内存和 20 GB 磁盘（[系统要求](../self-hosting/requirements.md)）。
- 一个域名，例如 `kaeso.example.com`，其 DNS 记录指向该服务器；80 和 443 端口向互联网开放，以便 Caddy 获取证书。
- 一个 OpenAI 或 Anthropic 的 API 密钥。

> [!TIP]
> 想在自己的电脑上试用，可以把域名设为 `localhost`，然后打开 `https://localhost`。此时 Caddy 用自己的本地证书颁发机构签发证书，浏览器会要求确认接受一次；[在自己的电脑上试用](../self-hosting/installation.md#在自己的电脑上试用)介绍如何让 curl 和 SDK 信任它。

## 下载 Compose 文件

每个版本都会发布一个 `compose.yaml`，其中固定了该版本的网关和控制台镜像。将它放到一个只有自己能读取的新文件夹中：

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

## 写入设置

创建 `.env`，写入域名和四个随机密码：

```bash
cat > .env <<EOF
KAESO_DOMAIN=kaeso.example.com
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
```

网关通过电子邮件发送登录链接、邀请和提醒，没有发信途径就不会启动。添加 SMTP 服务器；除非[另行设置](../self-hosting/email.md)，Kaeso 通过 587 端口和 STARTTLS 连接它：

```bash
cat >> .env <<'EOF'
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
EOF
```

如果没有邮件服务器，也可以让网关只记录邮件来试用 Kaeso。密码登录和设置链接照常可用；登录链接和邀请不会送达。

```bash
echo KAESO_MAIL_TRANSPORT=log >> .env
```

## 创建主密钥

主密钥用于加密 Kaeso 保存的服务商密钥。将一个主密钥生成到 `master-keys` 中，Compose 会把它作为 Docker secret 交给网关：

```bash
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

网关在容器内以非特权用户运行，因此该文件必须可读；文件夹的权限会挡住服务器上的其他用户。

> [!IMPORTANT]
> 将 `master-keys` 的副本保存在安全的地方，并与数据库备份分开存放。没有它，已保存的服务商密钥将无法解密，只能重新输入。

## 启动 Kaeso

```bash
docker compose up -d --wait
```

首次启动需要一两分钟，期间会下载镜像，网关会准备数据库。每个服务（包括 Caddy）都报告 healthy 后命令才返回；有服务达不到时，命令会失败并给出该服务的名称。Caddy 一开始提供服务就是 healthy，并在后台为新域名申请证书，因此在拿到证书之前，HTTPS 请求仍可能失败；`docker compose logs caddy` 会显示何时拿到证书，或者为什么拿不到。

## 创建第一个用户

设置链接用于创建第一个用户。在终端中输出它：

```bash
docker compose exec gateway kaeso setup-link
```

链接只能使用一次，须在 24 小时内使用。再次运行该命令会输出新链接，旧链接随即失效。

打开链接，输入姓名、电子邮件、一个 12 到 128 个字符的密码和组织名称，然后选择**设置 Kaeso**。完成后即成为实例管理员和第一个组织的所有者，该组织自带一个默认项目。

## 添加服务商密钥

服务商密钥属于组织。在侧边栏中打开**服务商**，选择**添加密钥**。选择 OpenAI 或 Anthropic，粘贴服务商提供的密钥，为它命名，例如 `Production`，然后选择**添加密钥**。

某个服务商的第一个密钥会成为其默认密钥，所有项目都会使用它。Kaeso 不会再次显示该密钥。详见[服务商密钥](../console/provider-keys.md)。

## 添加模型

请求只能使用项目目录中的模型。打开默认项目，选择**模型**，然后选择**添加模型**。选择服务商，搜索一个模型，例如 Anthropic 的 `claude-haiku-4-5` 或 OpenAI 的 `gpt-5-mini`，然后添加。它的价格、限制和能力来自 [models.dev](https://models.dev)；详见[模型和别名](../console/models-and-aliases.md)。

## 创建 API 密钥

在项目中选择 **API 密钥**，然后选择**创建密钥**。以将要使用它的应用命名，例如 `support-bot`，保留 90 天的有效期和访问权限**模型调用**，然后创建。

请立即复制密钥。它只显示一次，Kaeso 只保存它的哈希值：丢失的密钥无法找回，只能换新。

## 发送第一个请求

将密钥放入 shell，然后用 curl 调用模型：

```bash
export KAESO_API_KEY=sk-kso-…
curl https://kaeso.example.com/v1/chat/completions \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "messages": [{"role": "user", "content": "Write a haiku about gateways."}]}'
```

应用继续使用官方 SDK，只需更改基础 URL 和密钥。Python 版 OpenAI SDK（`pip install openai`）调用的是同一个 Claude 模型；Kaeso 会转换请求和回复。

```python
import os
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key=os.environ["KAESO_API_KEY"])

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

请使用自己添加的模型。然后在项目中打开**请求日志**：两个请求都在那里，附有状态、Token、费用和耗时。

## 下一步

| 目的 | 阅读 |
| --- | --- |
| 从 TypeScript 或用 Anthropic SDK 调用模型，并流式接收回复 | [调用模型](calling-models.md) |
| 将 Claude Code 或 Codex 指向自己的安装 | [编程工具](coding-tools.md) |
| 邀请成员，设置限额和预算 | [使用控制台](../console/index.md) |
| 通过自己的身份提供方登录 | [登录和单点登录](../self-hosting/sign-in.md) |
| 规划备份、升级和监控 | [自托管](../self-hosting/index.md) |
