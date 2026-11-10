---
title: MCP-Portal-API
description: Rufen Sie die Tools eines Projekts über MCP auf, und schlagen Sie Anmeldung, Versionen und Fehler des Portals nach.
order: 10
---

Das MCP-Portal jedes Projekts antwortet unter `/mcp/<Projekt-ID>` auf der Domain Ihrer Installation. Jeder MCP-Client ruft es mit einem API-Schlüssel mit der Fähigkeit **Tools** auf oder meldet sich als Person mit OAuth an. Diese Beispiele listen die Tools des Projekts, mit dem Schlüssel in `KAESO_API_KEY` und der ID des Projekts in `KAESO_PROJECT`:

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

## Anmeldung

| Aufrufer | So meldet er sich an |
| --- | --- |
| Ihre eigene App oder Ihr Agent | Ein API-Schlüssel des Projekts mit der Fähigkeit **Tools**, als `Authorization: Bearer sk-kso-…` |
| Ein Client wie claude.ai, ChatGPT oder Cursor | OAuth als Person: Der Client findet die Anmeldung über die erste Antwort des Portals, und die Person erlaubt ihn auf der Zustimmungsseite |

Eine Anfrage ohne gültige Anmeldung bekommt 401 mit `WWW-Authenticate: Bearer resource_metadata="…"`. Die Metadaten unter `/.well-known/oauth-protected-resource/mcp/<Projekt-ID>` nennen Kaeso als Autorisierungsserver und den Scope `tools`. Clients weisen sich mit einem Client-ID-Metadatendokument aus oder registrieren sich selbst. Ihre Tokens gelten nur beim Portal, für das sie ausgestellt wurden, und **Trennen** unter **Tools** beendet sie sofort.

In beiden Fällen läuft ein Aufruf als Person oder Dienstkonto: als Inhaber des Schlüssels oder als die Person, die den Client erlaubt hat. Sie brauchen `tools.use` im Projekt, und ihr Tool-Zugriff entscheidet, welche Tools sie sehen ([Tools](../console/tools.md#festlegen-wer-welches-tool-aufruft)).

## Versionen und Methoden

Das Portal spricht MCP über Streamable HTTP in den Versionen `2025-03-26`, `2025-06-18`, `2025-11-25` und `2026-07-28`. Version `2026-07-28` ist zustandslos: Senden Sie `MCP-Protocol-Version` bei jeder Anfrage, und `server/discover` listet die Versionen, die das Portal spricht. Ältere Versionen beginnen mit `initialize`, und eine Anfrage ohne den Header gilt als `2025-03-26`.

| Methode | Was sie tut |
| --- | --- |
| `server/discover` | Versionen, Fähigkeiten und Serverangaben des Portals |
| `initialize`, `ping` | Der Versionsabgleich der Versionen vor `2026-07-28` und eine Lebendprüfung |
| `tools/list` | Die Tools, die Sie aufrufen dürfen. Namen bestehen aus dem Kürzel des Servers, zwei Unterstrichen und dem Namen des Tools, etwa `x__create_post` |
| `tools/call` | Ruft ein Tool mit seinen `arguments` auf. Eine gestreamte Antwort des Servers wird als Ereignisse weitergereicht |
| `resources/list`, `resources/read` | Die [Ansichten von MCP Apps](#ansichten-von-mcp-apps) der Tools, die Sie aufrufen dürfen, und das HTML einer Ansicht |
| `resources/templates/list` | Immer leer |

Benachrichtigungen bekommen 202 ohne Inhalt. Andere Methoden bekommen `-32601`.

## Ansichten von MCP Apps

Ein Server, der mit [MCP Apps](https://modelcontextprotocol.io/extensions/apps/overview) gebaut ist, nennt für manche seiner Tools eine interaktive Ansicht, die Clients wie Claude im Gespräch zeigen. Das Portal reicht diese Ansichten weiter, sodass sie über Kaeso so funktionieren wie mit dem Server selbst:

- Kaeso teilt dem Server bei der Anmeldung und beim Auflisten der Tools mit, dass seine Clients Ansichten zeigen können (die Erweiterung `io.modelcontextprotocol/ui`), sodass Server, die Ansichten nur solchen Clients nennen, sie auch dem Portal nennen.
- Das `_meta` eines Tools erreicht den Client so, wie der Server es gelistet hat, mit der `ui://`-Adresse der Ansicht unter dem Kürzel des Servers, so wie es auch die Tool-Namen tragen: `ui://board/today` des Servers `x` heißt `ui://x/board/today`. `resources/list` listet die Ansichten der Tools, die Sie aufrufen dürfen, und `resources/read` holt eine mit Kaesos Anmeldung vom Server und reicht ihr HTML und ihr `_meta`, darunter die Content-Security-Policy der Ansicht, unverändert weiter. Die Ansicht eines Tools, das Sie nicht aufrufen dürfen, das aus ist oder auf eine Prüfung wartet, ist unbekannt (`-32002`).
- Eine Ansicht ruft die Tools ihres Servers mit den Namen des Servers auf. Hat nur ein Server mit Ansichten ein Tool dieses Namens, nimmt das Portal den Namen als dieses Tool, und der Aufruf durchläuft dieselben Prüfungen, Limits und Protokolle wie jeder andere.

Das HTML einer Ansicht kommt bei jedem Lesen vom Server, und die Prüfung eines geänderten Tools umfasst seine Definition, nicht seine Ansicht.

## Fehler

| Was passiert ist | Was der Client bekommt | Protokolliert als |
| --- | --- | --- |
| Kein solches Tool im Portal | JSON-RPC-Fehler `-32602`, der sagt, die Tools neu zu listen | Nicht protokolliert |
| Das Tool ist aus, oder Ihr Tool-Zugriff schließt es aus | JSON-RPC-Fehler `-32602`, der sagt, was zu tun ist | `tool_off`, `denied` |
| Das Tool wartet auf die Prüfung durch einen Admin | JSON-RPC-Fehler `-32602` | `tool_changed` |
| Eine Tool-Aufrufrate ist erreicht | HTTP 429 mit `Retry-After` und JSON-RPC-Fehler `-32000` | `limited` |
| Kaeso kann sich beim Server nicht anmelden oder ihn nicht erreichen | Ein Tool-Ergebnis mit `isError: true` und der Meldung von Kaeso | `upstream_auth`, `upstream_unreachable` |
| Der Server antwortet zu spät, zu umfangreich oder mit einem HTTP- oder Protokollfehler | Ein Tool-Ergebnis mit `isError: true` und der Meldung von Kaeso | `timeout`, `too_large`, `upstream_error` |
| Das Tool selbst schlägt fehl | Das eigene Ergebnis des Servers mit `isError: true`, unverändert | `upstream_error` |

Das Portal wartet `mcp_portal.call_timeout` auf eine Antwort, standardmäßig 60 Sekunden, und reicht höchstens `mcp_portal.response_limit` weiter, standardmäßig 4 MiB ([Konfiguration](../self-hosting/configuration.md)). Jeder Aufruf erscheint in den [Anfrageprotokollen](../console/request-logs.md) unter **Tools**.

## Nächste Schritte

- [MCP-Portal](../concepts/mcp-portal.md): Erfahren Sie, wie Server, Projekte, Verbindungen und Prüfungen zusammenspielen.
- [Tools](../console/tools.md): Schalten Sie Tools ein und verbinden Sie Clients in der Konsole.
- [MCP-Server](mcp.md): Verwalten Sie Ihre Installation selbst aus einem Coding-Agenten.
