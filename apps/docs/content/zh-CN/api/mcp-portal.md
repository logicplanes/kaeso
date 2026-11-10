---
title: MCP 门户 API
description: 通过 MCP 调用项目的工具，并查阅门户的登录方式、版本和错误。
order: 10
---

每个项目的 MCP 门户位于实例域名下的 `/mcp/<项目 ID>`。任何 MCP 客户端都可以用具有**工具**能力的 API 密钥调用它，或以个人身份通过 OAuth 登录。下面的示例使用 `KAESO_API_KEY` 中的密钥和 `KAESO_PROJECT` 中的项目 ID 列出项目的工具：

```bash tab="curl"
curl "https://kaeso.example.com/mcp/$KAESO_PROJECT" \
  -H "Authorization: Bearer $KAESO_API_KEY" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "MCP-Protocol-Version: 2025-11-25" \
  -d '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}'
```

```python tab="Python"
import asyncio
import os

from mcp import ClientSession
from mcp.client.streamable_http import streamablehttp_client

URL = f"https://kaeso.example.com/mcp/{os.environ['KAESO_PROJECT']}"


async def main():
    headers = {"Authorization": f"Bearer {os.environ['KAESO_API_KEY']}"}
    async with streamablehttp_client(URL, headers=headers) as (read, write, _):
        async with ClientSession(read, write) as session:
            await session.initialize()
            listed = await session.list_tools()
            print([tool.name for tool in listed.tools])


asyncio.run(main())
```

```ts tab="TypeScript"
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StreamableHTTPClientTransport } from "@modelcontextprotocol/sdk/client/streamableHttp.js";

const url = new URL(`https://kaeso.example.com/mcp/${process.env.KAESO_PROJECT}`);
const transport = new StreamableHTTPClientTransport(url, {
  requestInit: { headers: { Authorization: `Bearer ${process.env.KAESO_API_KEY}` } },
});
const client = new Client({ name: "my-agent", version: "1.0.0" });
await client.connect(transport);
const { tools } = await client.listTools();
console.log(tools.map((tool) => tool.name));
```

## 登录

| 调用方 | 登录方式 |
| --- | --- |
| 您自己的应用或智能体 | 项目中具有**工具**能力的 API 密钥，作为 `Authorization: Bearer sk-kso-…` 发送 |
| claude.ai、ChatGPT 或 Cursor 等客户端 | 以个人身份进行 OAuth：客户端从门户的第一个回答中找到登录方式，由本人在同意页面上允许它 |

没有有效登录的请求会收到 401，并带有 `WWW-Authenticate: Bearer resource_metadata="…"`。`/.well-known/oauth-protected-resource/mcp/<项目 ID>` 的元数据把 Kaeso 列为授权服务器，范围为 `tools`。客户端用客户端 ID 元数据文档表明身份，或自行注册。它们的令牌只在签发它们的门户有效，**工具**下的**断开连接**会立即使其失效。

无论哪种方式，调用都以个人或服务账号的身份运行：密钥的所有者，或允许该客户端的人。他们需要项目中的 `tools.use`，其工具访问权限决定他们能看到哪些工具（见[工具](../console/tools.md#决定谁可以调用哪个工具)）。

## 版本和方法

门户通过 Streamable HTTP 使用 MCP 版本 `2025-03-26`、`2025-06-18`、`2025-11-25` 和 `2026-07-28`。版本 `2026-07-28` 是无状态的：每个请求都要发送 `MCP-Protocol-Version`，`server/discover` 会列出门户支持的版本。较早的版本以 `initialize` 开始，没有该请求头的请求按 `2025-03-26` 处理。

| 方法 | 作用 |
| --- | --- |
| `server/discover` | 门户的版本、能力和服务器信息 |
| `initialize`、`ping` | `2026-07-28` 之前版本的版本协商，以及存活检查 |
| `tools/list` | 您可以调用的工具。名称由服务器的简称、两个下划线和工具名组成，例如 `x__create_post` |
| `tools/call` | 用工具的 `arguments` 调用一个工具。服务器的流式回答会以事件形式传递 |
| `resources/list`、`resources/read` | 您可以调用的工具的 [MCP Apps 视图](#mcp-apps-视图)，以及某个视图的 HTML |
| `resources/templates/list` | 始终为空 |

通知会收到 202，没有正文。其他方法会收到 `-32601`。

## MCP Apps 视图

使用 [MCP Apps](https://modelcontextprotocol.io/extensions/apps/overview) 构建的服务器会为部分工具指定交互视图，Claude 等客户端会在对话中显示这些视图。门户会传递这些视图，因此它们通过 Kaeso 使用时与直接连接服务器时一样：

- Kaeso 在登录服务器和列出工具时告诉服务器其客户端可以显示视图（`io.modelcontextprotocol/ui` 扩展），因此只向这类客户端列出视图的服务器也会向门户列出视图。
- 工具的 `_meta` 按服务器列出的样子传给客户端，视图的 `ui://` 地址会像工具名称一样加上服务器的简称：服务器 `x` 的 `ui://board/today` 变为 `ui://x/board/today`。`resources/list` 列出您可以调用的工具的视图，`resources/read` 用 Kaeso 的登录从服务器获取某个视图，并原样传递其 HTML 和 `_meta`（其中包括视图的内容安全策略）。您不能调用、已关闭或正在等待审核的工具的视图会被视为不存在（`-32002`）。
- 视图会用服务器自己的名称调用其服务器的工具。只有一个带视图的服务器有该名称的工具时，门户会把该名称视为这个工具，调用会经过与其他调用相同的检查、限额和日志。

每次客户端读取视图时，视图的 HTML 都来自服务器；对已变更工具的审核涵盖其定义，而不涵盖其视图。

## 错误

| 发生了什么 | 客户端收到什么 | 记录为 |
| --- | --- | --- |
| 门户中没有该工具 | JSON-RPC 错误 `-32602`，提示重新列出工具 | 不记录 |
| 工具已关闭，或您的工具访问权限不包含它 | JSON-RPC 错误 `-32602`，说明该怎么做 | `tool_off`、`denied` |
| 工具在等待管理员审核 | JSON-RPC 错误 `-32602` | `tool_changed` |
| 达到工具调用速率 | 带 `Retry-After` 的 HTTP 429 和 JSON-RPC 错误 `-32000` | `limited` |
| Kaeso 无法登录服务器，或无法访问它 | `isError: true` 的工具结果，附 Kaeso 的消息 | `upstream_auth`、`upstream_unreachable` |
| 服务器回答太慢、内容太多，或返回 HTTP 或协议错误 | `isError: true` 的工具结果，附 Kaeso 的消息 | `timeout`、`too_large`、`upstream_error` |
| 工具本身失败 | 服务器自己的 `isError: true` 结果，原样传递 | `upstream_error` |

门户等待回答的时间为 `mcp_portal.call_timeout`，默认 60 秒，最多传递 `mcp_portal.response_limit`，默认 4 MiB（见[配置](../self-hosting/configuration.md)）。每次调用都会出现在[请求日志](../console/request-logs.md)的**工具**下。

## 下一步

- [MCP 门户](../concepts/mcp-portal.md)：了解服务器、项目、连接和审核如何配合。
- [工具](../console/tools.md)：在控制台中开启工具并连接客户端。
- [MCP 服务器](mcp.md)：从编程智能体管理实例本身。
