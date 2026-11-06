---
title: Organization MCP servers
description: Add the organization's MCP servers, sign in to them and review the tools they change.
order: 4
---

**MCP servers** lists the servers your organization's projects can take tools from. Each row shows the server's address, its handle, how Kaeso is connected, its tools and the projects that use it. You need `mcp_servers.view` to see the page and `mcp_servers.manage` to change it ([MCP portal](../concepts/mcp-portal.md) explains the parts).

## Add a server

1. Open **MCP servers** and choose **Add server**.
2. Enter the server's Streamable HTTP address, such as `https://mcp.example.com/mcp`, and choose **Check address**. Kaeso shows what the server says it is, its MCP version, how it signs in and how many tools it lists.
3. Keep or change the name and the handle. The handle prefixes the server's tools for clients, such as `x__create_post`.
4. Choose how Kaeso signs in: **No sign-in**, **Secret header** or **OAuth**, and whether the account is **Shared** or **Each person signs in**.
5. For a shared secret header, enter the header's name and value. For shared OAuth, choose **Sign in to** the server: the sign-in opens in a new tab and the dialog shows **Connected as** the account once you're back.
6. Choose the tools projects should offer. Read-only tools are chosen, and destructive ones are marked. Keep **Hold changed tools for review** on unless you control the server.
7. Choose the projects that use the server and choose **Add server**. The chosen tools are on in those projects.

An address on a private network works only when the install allows its network in `mcp_servers.allowed_networks` ([configuration](../self-hosting/configuration.md)). Plain `http` works only on those networks.

When the server's authorization server takes neither client metadata documents nor registration, the dialog asks for a client id and, for a confidential client, its secret. Register Kaeso there with the redirect URI `https://kaeso.example.com/oauth/upstream/callback`.

## Sign in again or replace the secret

Choose a server to open its sheet. Under **Connection**, **Sign in again** starts a new OAuth sign-in, and **Replace secret** sets a new header. A server that refuses Kaeso's sign-in shows **Signed out** until someone does.

For a server where each person signs in, the sheet lists no connection: every person connects their own account under **Account** › **Connections**. The server's tools appear once the first person has connected.

## Review a changed tool

When a server changes a tool held for review, the sheet shows how many tools wait, and the tool shows **Changed** with **Review**. The review shows what changed in the title, description, input and output schema and hints, with the approved and the current definition's hash.

- **Approve change** approves the definition you reviewed. Projects that have the tool on offer it again.
- **Keep off** keeps the tool held and turns it off in every project.

If the server changes the tool again before you approve, nothing is approved, and you review the new change.

## Change a server

- **Check for new tools** asks the server for its tools now, instead of at the next check.
- **Choose projects** sets the projects that use the server. Each project still chooses its own tools.
- **Edit** renames the server, changes its handle or turns **Hold changed tools for review** on or off. A new handle renames every tool for clients.
- **Delete** removes the server with its sign-ins and tools. Every project's portal stops offering them.

## Next steps

- [Tools](tools.md): Turn a server's tools on in a project and connect clients.
- [MCP portal](../concepts/mcp-portal.md): Learn how connections, reviews and tool access fit together.
