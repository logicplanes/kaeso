---
title: Architektur
description: Wie Gateway, KPP, Konsole, UI-Bibliothek und Dokumentation in einem Repository zusammenspielen.
order: 2
---

```text
                     crates/kaeso-kpp, crates/kaeso-codecs
                                    │
apps ─▶ /v1 ─────────▶ gateway (crates/kaeso, crates/kaeso-*) ─▶ OpenAI, Anthropic
                         │    │
console ─▶ /platform/v1 ─┘    └─▶ openapi.json ─▶ the console's API client
   ▲
packages/ui, packages/tokens ─▶ apps/console, apps/docs, apps/showcase
```

- **Das Gateway nutzt KPP** für jede Modellanfrage. KPP dekodiert das Format der App in sein kanonisches Format und kodiert es für den Anbieter, und auf dem Rückweg ebenso. Das Gateway parst oder baut nie selbst das JSON eines Anbieters, und es hängt per Pfad innerhalb des Cargo-Workspaces von KPP ab, sodass eine Änderung an beiden ein einziger Pull Request ist.
- **Die Konsole nutzt die Platform API** und sonst nichts, sodass alles, was sie tut, auch ein Skript tun kann. Ihr API-Client wird aus `openapi.json` generiert, das das Gateway aus seinem Code erzeugt, sodass sich beide gemeinsam ändern.
- **Konsole, Dokumentation und Showcase werden mit der UI-Bibliothek gezeichnet.** Jedes sichtbare Element stammt aus `@logicplanes/kaeso-ui`, aufgebaut auf `@logicplanes/kaeso-tokens`, beide aus dem Workspace genutzt (`workspace:*`). Die E-Mails des Gateways verwenden dieselben Tokens.
- **Eine Version.** Alle Teile teilen die Versionsnummer des Releases, und die `compose.yaml` des Releases pinnt die Gateway- und Konsolen-Images genau dieses Releases.

## Seiten

| Seite | Inhalt |
| --- | --- |
| [Gateway](gateway.md) | Der Anfrageweg, die Crates, die Konfiguration und die Regeln, denen der Code des Gateways folgt |
| [Konsole](console.md) | Daten, Styling, Bundles und Fehler in der Web-Konsole |
| [Dokumentations-Website](docs-site.md) | Wie aus den Markdown-Seiten docs.kaeso.app wird |
| [E-Mails](emails.md) | Wie das Gateway die E-Mails baut, die es sendet |
| [KPP-Interna](../kpp/index.md) | Das kanonische Format und die Codecs |

## Der Rust-Workspace

| Crate | Aufgabe |
| --- | --- |
| `kaeso` | Das Binary des Gateways: seine Kommandozeile (`serve`, `migrate`, `master-key`, `user`, `setup-link`, `healthcheck`, `openapi`), Start und Herunterfahren. Das einzige Crate, das `anyhow` verwenden darf. |
| `kaeso-server` | Die HTTP-Oberflächen: Router für `/v1`, `/platform/v1`, Health und Metriken; Extraktoren, Fehlerzuordnung, das OpenAPI-Dokument, CSRF-Prüfungen, Request-IDs, Body-Limits; Konfiguration, Hintergrundjobs und Telemetrie. |
| `kaeso-core` | Domänenregeln ohne I/O: IDs, Rechte und Rollen, Schlüsselformat, Modellauflösung, der Katalog, Fähigkeitsprüfungen, Limits, Kosten, der Assembler für Stream-Antworten. Ohne Datenbanken getestet. Sein Ordner `data/` enthält den models.dev-Snapshot, der mit jedem Release ausgeliefert wird. |
| `kaeso-store` | Speicherung: PostgreSQL über sqlx, mit den Offline-Abfragedaten in `.sqlx/` im Wurzelverzeichnis des Repositorys, ClickHouse und Valkey mit Lua-Skripten; die Migrationen für PostgreSQL und ClickHouse. |
| `kaeso-upstream` | Transport zu den Anbietern: HTTP-Clients, Auth-Header, Basis-URLs, Timeouts, streamende Bodies und Abbruch. |
| `kaeso-crypto` | Envelope-Verschlüsselung (AES-256-GCM), der Master-Key-Ring, Passwörter, TOTP und Wrapper-Typen für Geheimnisse. |
| `kaeso-mail` | E-Mail: Zustellung per SMTP und jede E-Mail in einem Layout, in drei Sprachen ([E-Mails](emails.md)). |
| `kaeso-testkit` | Nur für Tests: Entwicklungs- und Testdatenbanken, der aus KPPs Fixtures gebaute Mock-Anbieter, ein Mock-Identitätsanbieter. |
| `kaeso-kpp`, `kaeso-codecs` | KPPs Typen und Codecs, auf crates.io veröffentlicht ([KPP-Interna](../kpp/index.md)). |
| `kpp-conformance` | Ein Server, der KPPs Encoder den offiziellen Python-SDKs für die [Konformitätssuite](../testing.md#sdk-konformität) bereitstellt. |
| `xtask` | Aufgaben im Repository: `cargo xtask models-dev` aktualisiert den models.dev-Snapshot, `cargo xtask record` [zeichnet Antworten von Anbietern auf](../kpp/fixtures.md#live-antworten-aufzeichnen). |

Alle Crates teilen Version, Edition und Lints des Workspaces in der `Cargo.toml` im Wurzelverzeichnis. Nur `kaeso-kpp` und `kaeso-codecs` werden veröffentlicht; die anderen setzen `publish = false`.

## Der TypeScript-Workspace

| Paket | Ordner | Aufgabe |
| --- | --- | --- |
| `kaeso-console` | `apps/console` | Die Web-Konsole, eine statische React-App, die neben dem Gateway ausgeliefert wird ([Konsole](console.md)) |
| `kaeso-docs` | `apps/docs` | Diese Dokumentation ([Dokumentations-Website](docs-site.md)) |
| `kaeso-showcase` | `apps/showcase` | Jede Komponente der UI-Bibliothek in jedem Zustand, mit ihren Anleitungen, unter [ui.kaeso.app](https://ui.kaeso.app) |
| `@logicplanes/kaeso-ui` | `packages/ui` | Die Komponentenbibliothek: React-Komponenten auf Base UI, gestaltet mit Tailwind CSS und den Tokens |
| `@logicplanes/kaeso-tokens` | `packages/tokens` | Die Design-Tokens als CSS-Variablen, ein Tailwind-Theme, TypeScript-Konstanten und JSON |

Die Apps lesen die Builds der Bibliotheken, daher läuft `pnpm build:packages` vor allem, was sie braucht. Die UI-Bibliothek liefert eigene Regeln und eine Komponenten-Registry für die Personen und Werkzeuge mit, die sie nutzen (`packages/ui/AGENTS.md`, `packages/ui/foundations/`, `packages/ui/ai/`); ihr Showcase dokumentiert jede Komponente.

## Im Produktivbetrieb

```text
browser, apps ─▶ https://kaeso.example.com ─▶ Caddy ─▶ /v1/*, /platform/* ─▶ gateway
                                                   └▶ everything else ─▶ console (static files)
gateway ─▶ PostgreSQL (state), ClickHouse (request logs), Valkey (counters, caches)
```

Die Konsole ist eine Sammlung statischer Dateien, ausgeliefert von einem eigenen kleinen Container auf derselben Domain wie das Gateway, sodass das Sitzungscookie ein First-Party-Cookie ist und kein CORS nötig ist. [Selbst hosten](../../self-hosting/index.md) beschreibt eine Installation.
