---
title: Tools
description: Choose the MCP tools a project's portal offers, decide who may call them, and connect clients.
order: 6
---

**Tools** in a project chooses which MCP servers and tools its portal offers, and connects clients such as claude.ai to it. You need `tools.view` to see the page, `tools.manage` to change it and `tools.use` to call tools ([MCP portal](../concepts/mcp-portal.md) explains the parts).

## Turn tools on

1. Open the project's **Tools** page. The **Tools** tab lists each server's tools, with the server's connection and how many of its tools are on.
2. Find a tool by searching its name or description, or filter by **Server**, **State** and **Hint**. Hints such as **Read-only** and **Destructive** are what the server claims, not guarantees.
3. Turn a tool on with its switch. Its state changes to **On**, and the portal offers it at once.

A tool that shows **Changed** waits for an admin to review its new definition under [Organization MCP servers](mcp-servers.md#review-a-changed-tool), and can't be turned on until then. Choose a tool to see its details, its input schema, its calls in the last 7 days and **See its calls**, which opens [request logs](request-logs.md) filtered to it.

## Use a server

The **Servers** tab lists the servers the project uses, with their connection, the share of tools on and their calls in the last 7 days.

- **Use a server** adds one of the organization's servers. Its tools start off. You also need `mcp_servers.view`.
- **Stop using** takes the server's tools out of the portal. Using it again starts with every tool off.

## Connect a client

The **Connect** tab, or **Connect a client** in the header, shows the portal address, such as `https://kaeso.example.com/mcp/proj_…`, with a copy button.

1. Choose the client: Claude, ChatGPT, Cursor or GitHub Copilot. The dialog shows that client's steps with the address filled in.
2. Add the address in the client. The client opens your browser, where you sign in to Kaeso.
3. Check what the client asks for, then choose **Connect**. The consent page lists the tools it can call, and the servers where you still need to connect your own account.

**Connected clients** lists every client someone connected, who connected it, where it receives its sign-ins, and when it was connected and last used. **Disconnect** ends its tokens at once.

Your own agent calls the portal with an API key that has the **Tools** ability. Turn it on when you [create a key](api-keys.md#create-a-key).

## Choose who may call which tool

Every member, service account and key may call every tool the project offers, as long as they have `tools.use`. To narrow it, open the member, the service account or the key and edit its **Tools** section: turn off **Every tool the project offers** and choose the tools. A key's tools are always within its owner's.

## Try tools in the Playground

In the [Playground](playground.md), the **Tools** switch sends the project's tools to the model. When the model asks for a call, the reply shows the tool and its arguments. **Run** calls the tool as you, and **Skip** tells the model you declined. Once every call is decided, the answers go back to the model. Playground calls are logged like any other.

## Next steps

- [Organization MCP servers](mcp-servers.md): Add servers, sign in to them and review changed tools.
- [MCP portal API](../api/mcp-portal.md): Look up the portal's sign-in, versions and errors.
- [Limits and budgets](limits-and-budgets.md): Cap tool calls per minute for a project, a member or a key.
