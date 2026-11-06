---
title: MCP portal API
description: Call a project's tools over MCP, and look up the portal's sign-in, versions and errors.
order: 6
---

Each project's MCP portal answers at `/mcp/<project id>` on your install's domain. Any MCP client calls it with an API key that has the **Tools** ability, or signs in as a person with OAuth. These examples list the project's tools with the key in `KAESO_API_KEY` and the project's id in `KAESO_PROJECT`:

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

## Sign-in

| Caller | How it signs in |
| --- | --- |
| Your own app or agent | An API key of the project with the **Tools** ability, as `Authorization: Bearer sk-kso-…` |
| A client such as claude.ai, ChatGPT or Cursor | OAuth as a person: the client finds the sign-in from the portal's first answer, and the person allows it on the consent page |

A request without a working sign-in gets 401 with `WWW-Authenticate: Bearer resource_metadata="…"`. The metadata at `/.well-known/oauth-protected-resource/mcp/<project id>` names Kaeso as the authorization server and the scope `tools`. Clients identify themselves with a client ID metadata document or register themselves. Their tokens work only at the portal they were issued for, and **Disconnect** under **Tools** ends them at once.

Either way, a call runs as a person or a service account: the key's owner, or the person who allowed the client. They need `tools.use` in the project, and their tool access decides which tools they see ([tools](../console/tools.md#choose-who-may-call-which-tool)).

## Versions and methods

The portal speaks MCP over Streamable HTTP in the versions `2025-03-26`, `2025-06-18`, `2025-11-25` and `2026-07-28`. Version `2026-07-28` is stateless: send `MCP-Protocol-Version` on every request, and `server/discover` lists the versions the portal speaks. Older versions start with `initialize`, and a request without the header is read as `2025-03-26`.

| Method | What it does |
| --- | --- |
| `server/discover` | The portal's versions, capabilities and server info |
| `initialize`, `ping` | The handshake of versions before `2026-07-28`, and a liveness check |
| `tools/list` | The tools you may call. Names are the server's handle, two underscores and the tool's name, such as `x__create_post` |
| `tools/call` | Calls one tool with its `arguments`. A server's streamed answer passes through as events |

Notifications get 202 and no body. Other methods get `-32601`.

## Errors

| What happened | What the client gets | Logged as |
| --- | --- | --- |
| No such tool in the portal | JSON-RPC error `-32602` that says to list the tools again | Not logged |
| The tool is off, or your tool access leaves it out | JSON-RPC error `-32602` that says what to do | `tool_off`, `denied` |
| The tool waits for an admin's review | JSON-RPC error `-32602` | `tool_changed` |
| A tool call rate is reached | HTTP 429 with `Retry-After` and JSON-RPC error `-32000` | `limited` |
| Kaeso can't sign in to the server, or can't reach it | A tool result with `isError: true` and Kaeso's message | `upstream_auth`, `upstream_unreachable` |
| The server answers too late, too much, or with an HTTP or protocol error | A tool result with `isError: true` and Kaeso's message | `timeout`, `too_large`, `upstream_error` |
| The tool itself fails | The server's own result with `isError: true`, unchanged | `upstream_error` |

The portal waits `mcp_portal.call_timeout` for an answer, 60 seconds by default, and passes on at most `mcp_portal.response_limit`, 4 MiB by default ([configuration](../self-hosting/configuration.md)). Every call appears in [request logs](../console/request-logs.md) under **Tools**.

## Next steps

- [MCP portal](../concepts/mcp-portal.md): Learn how servers, projects, connections and reviews fit together.
- [Tools](../console/tools.md): Turn tools on and connect clients in the console.
- [MCP server](mcp.md): Manage your install itself from a coding agent.
