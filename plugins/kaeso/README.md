# Kaeso plugin for Claude

Adds the Kaeso skill, and in Claude Code the connection to your self-hosted [Kaeso](https://kaeso.app) install's MCP server. The skill teaches your agent the server's tools. Your agent signs in through your browser, where you choose what it may do on the instance, in each organization and project, and on your account.

Every install has its own address, so the plugin has none built in. The plugin sends nothing anywhere itself; your agent talks to the address you give it.

## Claude Code

```text
/plugin install kaeso --marketplace logicplanes/kaeso
```

The plugin asks for your install's address followed by `/mcp`, such as `https://kaeso.example.com/mcp`. Run `/plugin configure kaeso@kaeso` to change it, then `/mcp`, choose `kaeso` and sign in.

Without the plugin, connect the server yourself:

```bash
claude mcp add --transport http --scope user kaeso https://kaeso.example.com/mcp
```

## Claude on the web, desktop and mobile, and Cowork

Claude asks for no address when you add a plugin, so add your install as a custom connector: open **Customize** › **Connectors**, choose **Add custom connector** and enter `Kaeso` and your address followed by `/mcp`. Or ask Claude to connect Kaeso: the skill gives you a link that fills the form in.

Installation, sign-in and every tool are described in the [MCP server documentation](https://docs.kaeso.app/api/mcp).
