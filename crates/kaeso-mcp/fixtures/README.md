# MCP message fixtures

Messages shaped like the examples of the [MCP specification](https://modelcontextprotocol.io/specification/2026-07-28), one per file, for both protocol eras: `2026-07-28` requests carry their version in `_meta`, `2025-11-25` and older ones start with `initialize`. `tests/fixtures.rs` checks that each one parses and is written back with every member intact, and the `mcp` fuzz target in `fuzz/` takes this folder as its seed corpus.

| File | Message |
| --- | --- |
| `tools-call-2026-07-28.json` | A stateless `tools/call` with protocol version, client info and capabilities in `_meta` |
| `tools-call-2025-11-25.json` | A `tools/call` of the session-based era, with a progress token |
| `initialize-2025-11-25.json` | The `initialize` request older clients start with |
| `initialized.json` | The notification that follows `initialize` |
| `server-discover.json` | `server/discover`, which every `2026-07-28` server answers |
| `server-discover-result.json` | Its result: versions, capabilities, server info, cache hints |
| `tools-list-result.json` | A `tools/list` result with an output schema, annotations, an `x-mcp-header` parameter and an MCP Apps `_meta` |
| `resources-read-2026-07-28.json` | A stateless `resources/read` |
| `input-required-result.json` | A multi round-trip interim result asking the client for input |
| `unsupported-version-error.json` | The error a server answers a version it doesn't speak with |
