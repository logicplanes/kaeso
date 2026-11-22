# Kaeso plugin for Claude

Adds the Kaeso skill, which teaches your agent the tools of your self-hosted [Kaeso](https://kaeso.app) install's MCP server and connects that server when it isn't connected yet. Your agent signs in through your browser, where you choose what it may do on the instance, in each organization and project, and on your account.

Every install has its own address, so the plugin has none built in and needs no settings. The plugin sends nothing anywhere itself; your agent talks to the address you give it.

## Claude Code

```text
/plugin install kaeso --marketplace logicplanes/kaeso
```

Ask Claude to connect Kaeso and give it your install's address, or connect the server yourself:

```bash
claude mcp add --transport http --scope user kaeso https://kaeso.example.com/mcp
```

Then run `/mcp`, choose `kaeso` and sign in.

## Claude on the web, desktop and mobile, and Cowork

Claude asks for no address when you add a plugin, so add your install as a custom connector: open **Customize** › **Connectors**, choose **Add custom connector** and enter `Kaeso` and your address followed by `/mcp`. Or ask Claude to connect Kaeso: the skill gives you a link that fills the form in.

Installation, sign-in and every tool are described in the [MCP server documentation](https://docs.kaeso.app/api/mcp).
