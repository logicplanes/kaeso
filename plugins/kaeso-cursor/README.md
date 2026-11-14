# Kaeso plugin for Cursor

Connects Cursor to your self-hosted [Kaeso](https://kaeso.app) install through its MCP server, and adds the Kaeso skill, which teaches your agent the server's tools. Your agent signs in through your browser, and you choose what it may do on the instance, in each organization and project, and on your account.

Install **Kaeso** from the Cursor Marketplace. It asks for the address of your Kaeso install as `KAESO_URL`, such as `https://kaeso.example.com`, without a trailing slash. Change it under **Plugins** › **Configure** in the Cursor dashboard.

If Cursor doesn't ask, or you only want the connection, open **Account** › **Agents** in your Kaeso console and choose **Add to Cursor**, which adds the server with your install's address.

Installation, sign-in and every tool are described in the [MCP server documentation](https://docs.kaeso.app/api/mcp).
