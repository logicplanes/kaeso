---
title: 快速开始
description: 用 Docker Compose 运行 Kaeso，登录，连接服务商并发送第一个请求。
order: 1
---

本页带你从一台空服务器走到一个通过 Kaeso 调用模型的应用。它用一个 Compose 文件启动整个安装：网关、控制台、PostgreSQL、ClickHouse、Valkey 和 Caddy，其中 Caddy 会自动为你的域名获取 TLS 证书。

## 准备工作

- 一台装有 Docker Engine 和 Docker Compose 2.24 或更高版本的服务器，至少 2 个 vCPU、4 GB 内存和 20 GB 磁盘。详见[系统要求](../self-hosting/requirements.md)。
- 一个域名，例如 `kaeso.example.com`，其 DNS 记录指向该服务器。
- 向互联网开放 80 和 443 端口，以便 Caddy 获取证书。
- 一个 OpenAI 或 Anthropic 的密钥。

> [!TIP]
> 如果想在自己的电脑上试用 Kaeso，请在下文中将域名设为 `localhost`，然后打开 `https://localhost`。Caddy 会使用其本地证书颁发机构签发的证书，浏览器会要求确认接受一次。

## 下载 Compose 文件

每个 Kaeso 版本都附带一个 `compose.yaml`，其中固定了该版本的网关和控制台镜像。将它放到一个只有自己能读取的文件夹中：

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

## 写入设置

在 `compose.yaml` 旁边创建 `.env`，写入域名和两个随机的数据库密码：

```bash
printf 'KAESO_DOMAIN=kaeso.example.com\nPOSTGRES_PASSWORD=%s\nCLICKHOUSE_PASSWORD=%s\n' "$(openssl rand -hex 24)" "$(openssl rand -hex 24)" > .env
```

网关通过电子邮件发送登录链接、邀请和提醒，没有发送邮件的途径时会拒绝启动。将 SMTP 服务器添加到 `.env`：

```bash
KAESO_SMTP_HOST=smtp.example.com
KAESO_SMTP_USER=kaeso
KAESO_SMTP_PASSWORD=…
KAESO_SMTP_FROM=Kaeso <kaeso@example.com>
```

如果想在没有邮件服务器的情况下试用 Kaeso，可以改为添加 `KAESO_MAIL_TRANSPORT=log`。这样网关只会记录本应发送一封邮件；用密码登录仍然可用。所有选项见[电子邮件](../self-hosting/email.md)。

## 创建主密钥

主密钥用于加密将要保存的服务商密钥。将一个主密钥生成到文件 `master-keys` 中，Compose 文件会把它作为 Docker secret 交给网关：

```bash
docker run --rm ghcr.io/logicplanes/kaeso-gateway:latest master-key generate > master-keys
chmod 644 master-keys
```

网关在容器内以非特权用户运行，必须能读取该文件；文件夹则对服务器上的其他用户保持封闭。

> [!IMPORTANT]
> 将 `master-keys` 的副本保存在安全的地方，并与数据库备份分开存放。没有它，已保存的服务商密钥将无法解密，只能重新输入。

## 启动 Kaeso

```bash
docker compose up -d
```

首次启动需要一两分钟：下载镜像、启动数据库、网关准备数据库，以及 Caddy 申请证书。服务就绪后，`docker compose ps` 会将其显示为 healthy。

## 通过设置链接登录

安装中还没有用户时，网关会在日志中输出一个设置链接：

```bash
docker compose logs gateway | grep setup_url
```

在浏览器中打开该链接。它只能使用一次，有效期为 24 小时。输入姓名、电子邮件、一个 12 到 128 个字符的密码以及组织名称，然后选择**设置 Kaeso**。

现在你是该安装的第一位实例管理员，也是其第一个组织的所有者，该组织自带一个默认项目。

如果链接已过期或丢失，可以输出一个新链接；旧链接随即失效：

```bash
docker compose exec gateway kaeso setup-link
```

## 添加服务商密钥

服务商密钥属于组织。在侧边栏中打开**服务商**，选择**添加密钥**。选择 OpenAI 或 Anthropic，粘贴服务商提供的密钥，为其命名，例如 `Production`，然后选择**添加密钥**。

某个服务商的第一个密钥会成为其默认密钥，所有项目都会使用它。Kaeso 不会再次显示该密钥。详见[服务商密钥](../console/provider-keys.md)。

## 添加模型

请求只能使用项目目录中的模型。打开默认项目，选择**模型**，然后选择**添加模型**。搜索一个模型，例如 `claude-haiku-4-5` 或 `gpt-5-mini`，选中并添加。它的价格、限制和能力来自 [models.dev](https://models.dev)。详见[模型和别名](../console/models-and-aliases.md)。

## 创建 API 密钥

在项目中选择 **API 密钥**，然后选择**创建密钥**。以将要使用它的应用命名，例如 `support-bot`，保留 90 天的有效期和访问权限**模型调用**（用于调用模型 API），然后创建。

> [!IMPORTANT]
> 密钥只显示一次。Kaeso 只保存它的哈希值，因此丢失的密钥无法找回；请改为创建一个新密钥。

## 发送第一个请求

应用使用官方 SDK，只需更改基础 URL 和密钥。任何 SDK 都可以调用目录中的任何模型：Kaeso 会转换请求和回复。

| SDK | 基础 URL |
| --- | --- |
| OpenAI | `https://kaeso.example.com/v1` |
| Anthropic | `https://kaeso.example.com` |

使用 Python 版 OpenAI SDK（`pip install openai`）：

```python
from openai import OpenAI

client = OpenAI(base_url="https://kaeso.example.com/v1", api_key="sk-kso-…")

response = client.responses.create(model="claude-haiku-4-5", input="Write a haiku about gateways.")
print(response.output_text)
```

使用 Python 版 Anthropic SDK（`pip install anthropic`）：

```python
from anthropic import Anthropic

client = Anthropic(base_url="https://kaeso.example.com", api_key="sk-kso-…")

message = client.messages.create(
    model="claude-haiku-4-5",
    max_tokens=200,
    messages=[{"role": "user", "content": "Write a haiku about gateways."}],
)
print(message.content[0].text)
```

两个示例都调用 `claude-haiku-4-5`，因此 OpenAI SDK 调用的是一个 Claude 模型；也可以使用添加到项目中的任何模型。在项目中打开**请求日志**：每个请求都列在那里，附有状态、模型、Token、费用和耗时。

## 下一步

| 目的 | 阅读 |
| --- | --- |
| 从 TypeScript 或 curl 调用模型、流式接收回复并再次找到请求 | [调用模型](calling-models.md) |
| 将 Claude Code 或 Codex 指向自己的安装 | [编程工具](coding-tools.md) |
| 邀请成员，设置限额和预算 | [使用控制台](../console/index.md) |
| 规划备份、升级和监控 | [自托管](../self-hosting/index.md) |
