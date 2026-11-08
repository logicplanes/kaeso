---
title: 快速开始
description: 用 Docker Compose 安装 Kaeso，连接服务商并发送第一个请求。
order: 1
---

在服务器上安装 Kaeso，连接 OpenAI、Anthropic 等模型服务商，并通过它发送第一个请求。

## 准备工作

- 一台服务器，2 个 vCPU、4 GB 内存和 20 GB 磁盘，并装有 Docker Engine 和 Docker Compose 2.24 或更高版本（[系统要求](../self-hosting/requirements.md)）。在 Linux 上，如果缺少 Docker，安装脚本会提议为您安装。
- 一个域名，例如 `kaeso.example.com`，其 DNS 记录指向该服务器。80 和 443 端口必须向互联网开放，以便 Caddy 获取证书。
- 一个 [Kaeso 所调用的模型服务商](../concepts/supported-providers.md)（如 OpenAI 或 Anthropic）的 API 密钥。

> [!TIP]
> 想在自己的电脑上试用，把域名设为 `localhost`，然后打开 `https://localhost`。此时 Caddy 自行签发证书，浏览器会要求您确认接受一次。[在自己的电脑上试用](../self-hosting/installation.md#在自己的电脑上试用)介绍如何让 curl 和 SDK 信任它。

## 安装 Kaeso

在服务器上运行安装脚本。它会创建一个只有您能读取的 `kaeso` 文件夹，并把最新版本的 `compose.yaml` 下载到其中。然后它写入包含您的域名和随机密码的 `.env`，创建主密钥并启动所有服务：

```bash
curl -fsSL https://kaeso.app/install.sh | sh -s -- --domain kaeso.example.com
```

安装之前，如果 Docker 已停止，安装脚本会启动它，并检查您的域名是否有 DNS 记录。首次启动需要一两分钟，期间会下载镜像。随后安装脚本会检查 Caddy 是否已获得该域名的证书；如果没有，它会说明证书颁发机构的答复。最后它会输出设置链接；在您自己的电脑上运行时，还会在浏览器中打开该链接，`--no-open` 可阻止打开。在您[添加 SMTP 服务器](../self-hosting/email.md)之前，网关只会把邮件写入日志，因此登录链接和邀请不会送达。[手动安装](../self-hosting/installation.md#手动安装)列出了安装脚本执行的每个步骤。

在实例文件夹中再次运行安装脚本即可重新启动 Kaeso，它会保留设置和主密钥。安装脚本还会开启[更新器](../self-hosting/upgrades.md#更新器)，这样**实例** › **更新**下的**更新到 …**就能安装新版本。一台机器只运行一个 Kaeso，因此安装脚本会拒绝在其他文件夹中进行第二次安装。

> [!IMPORTANT]
> 将 `kaeso/master-keys` 的副本保存在安全的地方，并与数据库备份分开存放。没有它，Kaeso 无法解密已保存的服务商密钥，您只能重新输入。

## 创建第一个用户

安装脚本输出的设置链接用于创建第一个用户。链接只能使用一次，须在 24 小时内使用。要输出新链接并让旧链接失效，请在 `kaeso` 文件夹中运行：

```bash
docker compose exec gateway kaeso setup-link
```

打开链接，输入您的姓名、电子邮件、一个 12 到 128 个字符的密码和组织名称，然后选择**设置 Kaeso**。完成后，您就是实例管理员和第一个组织的所有者，Kaeso 会打开该组织的**概览**。该组织中还没有项目。

## 添加服务商密钥

服务商密钥属于组织，每个项目都使用服务商的默认密钥。

1. 在侧边栏中打开**服务商**，选择**添加服务商密钥**。
2. 选择服务商，粘贴它提供的密钥。
3. 为它命名，例如 `Production`，然后选择**添加密钥**。

某个服务商的第一个密钥会成为其默认密钥。Kaeso 不会再次显示该密钥；详见[服务商密钥](../console/provider-keys.md)。

## 创建项目

模型、API 密钥和请求日志都属于某个项目。

1. 在组织的**概览**中，选择**创建项目**。
2. 按用途为它命名，例如 `Support bot`，然后选择**创建项目**。

Kaeso 会打开新项目，您是它的管理员；详见[组织和项目](../console/organizations-and-projects.md#创建项目)。

## 添加模型

请求只能使用项目目录中的模型。

1. 在项目中，选择**模型**，然后选择**添加模型**。
2. 选择服务商，搜索一个模型，例如 Anthropic 的 `claude-haiku-4-5` 或 OpenAI 的 `gpt-5-mini`。
3. 添加它。

模型的价格、限制和能力来自 [KPMR](../concepts/kpmr.md)（Kaeso Provider & Model Registry）；详见[模型和别名](../console/models-and-aliases.md)。

## 创建 API 密钥

1. 在项目中选择 **API 密钥**，然后选择**创建密钥**。
2. 以将要使用它的应用命名，例如 `support-bot`。
3. 保留 90 天的有效期和访问权限**模型调用**，然后创建密钥。

请立即复制密钥。Kaeso 只显示它一次，并且只保存它的哈希值，因此丢失的密钥无法找回，只能换新。把它放入 shell：

```bash
export KAESO_API_KEY=sk-kso-…
```

## 发送第一个请求

用 curl 调用模型，或使用 Python 版（`pip install openai`）或 TypeScript 版（`npm install openai`）OpenAI SDK。OpenAI SDK 调用的是您添加的 Claude 模型：Kaeso 会转换请求和回复。

```bash tab="curl"
curl https://kaeso.example.com/v1/responses \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "content-type: application/json" \
  -d '{"model": "claude-haiku-4-5", "input": "Write a haiku about gateways."}'
```

```python tab="Python"
import os
from openai import OpenAI

client = OpenAI(
    base_url="https://kaeso.example.com/v1",
    api_key=os.environ["KAESO_API_KEY"],
)

response = client.responses.create(
    model="claude-haiku-4-5",
    input="Write a haiku about gateways.",
)
print(response.output_text)
```

```ts tab="TypeScript"
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://kaeso.example.com/v1",
  apiKey: process.env.KAESO_API_KEY,
});

const response = await client.responses.create({
  model: "claude-haiku-4-5",
  input: "Write a haiku about gateways.",
});
console.log(response.output_text);
```

如果您添加的是其他模型，请改用它的 ID。回复中是模型写的俳句。在项目中打开**请求日志**，可以看到这个请求及其状态、Token、费用和耗时。

## 下一步

- [调用模型](calling-models.md)：同时使用 Anthropic 或 Google Gen AI SDK，流式接收回复，并在日志中找到请求。
- [编程工具](coding-tools.md)：将 Claude Code 或 Codex 指向您的实例。
- [使用控制台](../console/index.md)：邀请成员，设置限额和预算。
- [自托管](../self-hosting/index.md)：规划登录、备份、升级和监控。
