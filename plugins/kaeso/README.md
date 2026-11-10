# Kaeso plugin for Claude Code

Adds the Kaeso skill to Claude Code. It teaches your agent the tools of your self-hosted [Kaeso](https://kaeso.app) install's MCP server. The plugin itself needs no settings: when the server isn't connected yet, the skill asks for your install's address and connects it.

```text
/plugin install kaeso --marketplace logicplanes/kaeso
```

You can also connect the server yourself:

```bash
claude mcp add --transport http --scope user kaeso https://kaeso.example.com/mcp
```

Then run `/mcp` and choose `kaeso` to sign in. Your browser opens your install's console, where you choose what the agent may do on the instance, in each organization and project, and on your account.

Installation, sign-in and every tool are described in the [MCP server documentation](https://docs.kaeso.app/api/mcp).
