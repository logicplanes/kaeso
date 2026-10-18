---
title: Abhängigkeiten und Lizenzen
description: Jede direkte Abhängigkeit, warum es sie gibt, ihre Lizenz und die Prüfungen, die die Richtlinie durchsetzen.
order: 8
---

Kaeso steht unter der MIT-Lizenz, und alles, was es ausliefert, hängt nur von permissiv lizenziertem Code ab. Eine neue Abhängigkeit bekommt im selben Pull Request eine Zeile auf dieser Seite, die sagt, warum sie nötig ist und unter welcher Lizenz sie steht.

## Lizenzrichtlinie

| Was ausgeliefert wird | Erlaubte Lizenzen | Geprüft von |
| --- | --- | --- |
| Das Gateway | MIT, MIT-0, Apache-2.0 (auch mit der LLVM-Ausnahme), BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Unicode-DFS-2016, Zlib und die [Ausnahmen](#ausnahmen) unten | `cargo deny check` mit der `deny.toml` im Wurzelverzeichnis |
| `kaeso-kpp`, `kaeso-codecs` | Dieselbe Liste ohne Ausnahmen; `tokio`, `reqwest`, `hyper` und `axum` sind verboten | `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` mit `crates/kaeso-codecs/deny.toml` |
| `@logicplanes/kaeso-ui`, `@logicplanes/kaeso-tokens` | MIT, ISC, BSD-2-Clause, BSD-3-Clause, Apache-2.0, 0BSD, für ihre Produktionsabhängigkeiten | `pnpm licenses:check` (`scripts/licenses.mjs`) |
| Die Konsole und die Dokumentation | MIT, ISC, BSD-2-Clause, BSD-3-Clause, Apache-2.0, 0BSD, für jedes Paket, dessen Code in ihrem Build steckt | `pnpm licenses:check`, nach `pnpm build` |

`cargo deny check` lehnt außerdem Crates aus unbekannten Registries oder Git-Repositorys ab, ebenso Wildcard-Versionen, zurückgezogene Crates und Crates mit Sicherheitsmeldungen. GPL-, LGPL- und AGPL-Code ist in nichts, was Kaeso ausliefert, je erlaubt. npm-Pakete sind in den `package.json`-Dateien auf exakte Versionen gepinnt; Cargo-Versionen sind in `Cargo.lock` festgeschrieben.

## Rust

### Das Gateway

| Crate | Wofür | Lizenz |
| --- | --- | --- |
| `kaeso-kpp`, `kaeso-codecs` | KPP-Typen und jedes Wire-Format, aus demselben Workspace | MIT |
| `tokio` | Die asynchrone Runtime | MIT |
| `axum` | Der HTTP-Server | MIT |
| `hyper`, `hyper-util` | Die Verbindungen des Servers: Header-Read-Timeout, HTTP/2-Keepalive-Pings | MIT |
| `tower` | Middleware: Timeouts, Body-Limits | MIT |
| `reqwest` | Transport zu den Anbietern mit rustls, HTTP/2 und streamenden Bodies | MIT OR Apache-2.0 |
| `futures`, `tokio-util`, `bytes` | Streams, Abbruch, Byte-Puffer | MIT (`bytes`, `tokio-util`), MIT OR Apache-2.0 (`futures`) |
| `sqlx` | PostgreSQL mit zur Compile-Zeit geprüften Abfragen und Migrationen | MIT OR Apache-2.0 |
| `clickhouse` | Der offizielle ClickHouse-Client, für Anfrageprotokolle und zum Löschen von Testdatenbanken in `kaeso-testkit` | MIT OR Apache-2.0 |
| `redis` | Der Valkey-Client, mit einem Connection Manager und Lua-Skripten | BSD-3-Clause |
| `serde`, `serde_json` | Serialisierung | MIT OR Apache-2.0 |
| `humantime-serde` | Zeitdauern in der Konfiguration | MIT OR Apache-2.0 |
| `toml` | Die Konfigurationsdatei und das models.dev-Supplement | MIT OR Apache-2.0 |
| `clap` | Die Kommandozeilen von `kaeso` und `cargo xtask` | MIT OR Apache-2.0 |
| `tracing`, `tracing-subscriber` | Strukturierte JSON-Logs | MIT |
| `metrics`, `metrics-exporter-prometheus` | Prometheus-Metriken | MIT (`metrics`), MIT AND Apache-2.0 (`metrics-exporter-prometheus`) |
| `utoipa`, `utoipa-axum` | Das generierte OpenAPI-Dokument | MIT OR Apache-2.0 |
| `aes-gcm` | Envelope-Verschlüsselung von Anbieter-Schlüsseln | Apache-2.0 OR MIT |
| `argon2` | Passwort-Hashing (Argon2id) | MIT OR Apache-2.0 |
| `sha2`, `subtle` | Hashes von Schlüsseln und Tokens, der Hash des models.dev-Snapshots in `cargo xtask models-dev`, Vergleiche in konstanter Zeit | MIT OR Apache-2.0 (`subtle`: BSD-3-Clause) |
| `rand` | Zufällige Geheimnisse und Tokens | MIT OR Apache-2.0 |
| `zeroize`, `secrecy` | Löschen und Schwärzen von Geheimnissen im Speicher | Apache-2.0 OR MIT |
| `totp-rs` | TOTP (RFC 6238) | MIT |
| `openidconnect` | Single Sign-on mit OpenID Connect; seine HTTP-Anfragen laufen über den eigenen, gegen SSRF abgesicherten Client des Gateways | MIT |
| `lettre` | SMTP | MIT |
| `maud` | Das HTML der E-Mails: zur Compile-Zeit geprüfte Templates, die jeden Wert escapen ([E-Mails](architecture/emails.md)) | MIT OR Apache-2.0 |
| `resvg` (nur Tests) | Rendert in einem Test die SVGs der Wortmarke, um zu prüfen, dass die Bilder der E-Mails aktuell sind | Apache-2.0 OR MIT |
| `uuid` | UUIDv7-Primärschlüssel | Apache-2.0 OR MIT |
| `time` | Zeitstempel in UTC | MIT OR Apache-2.0 |
| `rust_decimal` | Geld, nie ein Float | MIT |
| `rust_decimal_macros` (nur Tests) | Decimal-Literale in Tests | MIT |
| `regex` (nur Tests) | Prüft, dass das dokumentierte Dezimalmuster und der handgeschriebene Parser in `kaeso-core` übereinstimmen | MIT OR Apache-2.0 |
| `base64`, `url`, `ipnet`, `percent-encoding` | Helfer für Daten | MIT OR Apache-2.0 |
| `idna` | Erlaubte E-Mail-Domains, verglichen in ihrer ASCII-Form, gleich in welcher Schreibweise oder Schrift sie eingegeben wurden | MIT OR Apache-2.0 |
| `moka` | Prozessinterne Caches mit Ablaufzeit | (MIT OR Apache-2.0) AND Apache-2.0 |
| `thiserror` | Typisierte Fehler in den Bibliotheken | MIT OR Apache-2.0 |
| `anyhow` | Fehler nur im Binary | MIT OR Apache-2.0 |
| `insta`, `proptest` | Tests | MIT OR Apache-2.0 (`insta`: Apache-2.0) |
| `http-body-util` (nur Tests) | Lesen von Antwort-Bodies in den Tests des Servers | MIT |
| `rsa`, `chrono` | Der Mock-Identitätsanbieter in `kaeso-testkit`, nur in Tests | MIT OR Apache-2.0 |

### Der Updater

Der Updater (`crates/kaeso-updater`) nutzt `tokio`, `axum`, `reqwest`, `serde`, `serde_json`, `tracing`, `secrecy`, `subtle`, `sha2`, `base64`, `url`, `time`, `futures` und `thiserror` wie das Gateway, und diese:

| Crate | Wofür | Lizenz |
| --- | --- | --- |
| `kaeso-update` | Das Release-Manifest, die Update-Regeln und das Protokoll des Updaters, aus demselben Workspace; auch das Gateway nutzt es | MIT |
| `semver` | Release-Versionen vergleichen, im Gateway und im Updater | MIT OR Apache-2.0 |
| `sigstore` (nur Feature `verify`) | Die Build-Provenance-Attestierungen des Release-Workflows prüfen: die Zertifikatskette bis zur Zertifizierungsstelle von Sigstore, den Certificate-Transparency-Nachweis, die Identität des Workflows, die DSSE-Signatur und den Eintrag im Transparenzlog | Apache-2.0 |
| `rustls-pki-types` | Die Zertifikate der Vertrauensbasis, für `sigstore` | MIT OR Apache-2.0 |
| `aws-lc-rs` | Den signierten Zeitstempel des Transparenzlogs mit dem Rekor-Schlüssel prüfen, den `sigstore` auslässt | ISC AND (Apache-2.0 OR ISC) |

Das Self-Hosting-Paket führt [`wollomatic/socket-proxy`](https://github.com/wollomatic/socket-proxy) (MIT, mit Teilen unter Apache-2.0) als Docker-Socket-Proxy des Updaters aus, nur im Profil `updater`.

### KPP

Der Bibliothekscode von `kaeso-kpp` und `kaeso-codecs` darf nur `serde`, `serde_json`, `bytes`, `thiserror`, `indexmap`, `base64` und `mime` verwenden; alles andere braucht die Zustimmung des Maintainers und hier eine Zeile. Genutzt werden:

| Crate | Verwendet von | Warum | Lizenz |
| --- | --- | --- | --- |
| `serde` (mit `derive`) | Beiden | Die stabile JSON-Form der KPP-Typen; Wire-Structs | MIT OR Apache-2.0 |
| `serde_json` (mit `raw_value`, `float_roundtrip`) | Beiden | JSON; `RawValue` für byte-genaue Roh-Payloads; `float_roundtrip`, damit ein Float genau zu dem Wert geparst wird, der ausgegeben wurde, worauf die Property-Tests von KPP angewiesen sind | MIT OR Apache-2.0 |
| `indexmap` (mit `serde`) | Beiden | Maps mit erhaltener Reihenfolge für Extras | Apache-2.0 OR MIT |
| `thiserror` | Beiden | Fehlertypen | MIT OR Apache-2.0 |
| `base64` | `kaeso-codecs` | Data-URLs und Embedding-Vektoren | MIT OR Apache-2.0 |

Tests, Benchmarks und Werkzeuge dürfen mehr verwenden, nie als normale Abhängigkeit einer Bibliothek:

| Crate | Verwendet von | Warum | Lizenz |
| --- | --- | --- | --- |
| `insta` (mit `json`) | Tests von `kaeso-codecs` | Snapshots der dialektübergreifenden Matrix | Apache-2.0 |
| `proptest` | Tests von `kaeso-codecs` | Property-Tests: Aufteilung von SSE-Chunks, KPP-Round-Trips | MIT OR Apache-2.0 |
| `criterion` | Benchmarks von `kaeso-codecs` | Benchmarks | Apache-2.0 OR MIT |
| `libfuzzer-sys` | `fuzz/`, ein eigener Workspace | Die cargo-fuzz-Targets | (MIT OR Apache-2.0) AND NCSA |
| `axum`, `tokio`, `serde`, `serde_json` | `crates/kpp-conformance`, nie veröffentlicht | Der Konformitätsserver, nie im Abhängigkeitsgraphen der Bibliothek, was die `deny.toml` von KPP erzwingt | MIT (`axum`, `tokio`), MIT OR Apache-2.0 |

### Ausnahmen

Transitive Crates unter einer permissiven Lizenz, die nicht auf der Liste der erlaubten steht. Jede Ausnahme steht in der `deny.toml` im Wurzelverzeichnis und gilt nur für dieses Crate; die Richtlinie von KPP hat keine.

| Crate | Eingebunden über | Lizenz | Warum sie akzeptabel ist |
| --- | --- | --- | --- |
| `quoted_printable` | `lettre` | 0BSD | Gleichwertig mit Public Domain, ganz ohne Bedingungen |
| `webpki-roots`, `webpki-root-certs` | `sqlx`, `reqwest` (rustls) | CDLA-Permissive-2.0 | Mozillas Liste von CA-Zertifikaten als Daten; permissiv, kein Copyleft |
| `xxhash-rust` | `redis` | BSL-1.0 | Die Boost Software License: permissiv, Namensnennung nur in Quellcode-Distributionen |

### Ignorierte Sicherheitsmeldungen

| Meldung | Crate | Warum sie nicht zutrifft |
| --- | --- | --- |
| [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071) (Marvin-Angriff) | `rsa`, über `openidconnect` | Der Timing-Seitenkanal betrifft Operationen mit privaten RSA-Schlüsseln. Das Gateway prüft nur Signaturen von ID-Tokens mit den öffentlichen Schlüsseln der Identitätsanbieter; private Schlüssel nutzt nur der Mock-Identitätsanbieter in `kaeso-testkit`, in Tests. Die Meldung wird erneut geprüft, sobald `rsa` einen Fix ausliefert. |

### Erwogen und nicht verwendet

| Crate | Warum nicht |
| --- | --- |
| `figment` | Die Konfiguration wird von einer einzigen Einstellungstabelle gesteuert, deren Tests Namen, Standardwerte und Beschreibungen prüfen und eine geprüfte Referenz rendern ([Konfiguration](architecture/gateway.md#konfiguration)). Mit dieser Tabelle ist das Zusammenführen von Standardwerten, TOML-Datei und `KAESO_*`-Variablen ein paar Zeilen über `toml`, mit Fehlern, die immer die Einstellung nennen; figment brächte eine zweite Quelle der Wahrheit für Schlüssel und Standardwerte mit. |

## TypeScript

### Die UI-Bibliothek

| Paket | Lizenz | Warum |
| --- | --- | --- |
| `@base-ui/react` | MIT | Die Headless-Schicht: Semantik, Fokusverwaltung, Tastaturbedienung und Positionierung für jede interaktive Komponente |
| `@logicplanes/kaeso-tokens` | MIT | Die Werte, die jede Komponente verwendet, aus `packages/tokens` |
| `@tanstack/react-table` | MIT | Sortierung, Größenänderung, Auswahl und Spaltenzustand für `DataTable` |
| `@tanstack/react-virtual` | MIT | Virtualisierte Zeilen in `DataTable` und `List` ab 100 Zeilen |
| `clsx`, `tailwind-merge` | MIT | Zusammensetzen von Klassen; `tailwind-merge` ist mit `createTailwindMerge` nur für die Klassengruppen des Themes eingerichtet, das nur aus Tokens besteht, sodass es klein bleibt und die Layout-Klassen eines Nutzers sauber Vorrang haben |
| `d3-array`, `d3-scale`, `d3-shape`, `d3-time`, `d3-sankey` | ISC, BSD-3-Clause (`d3-sankey`) | Skalen, Formen und Layouts für die Diagramme, gezeichnet als React-SVG |
| `marked` | MIT | Der Markdown-Lexer hinter `Markdown` (`@logicplanes/kaeso-ui/markdown`, `@logicplanes/kaeso-ui/playground`, `markdown` von `ConversationView`): nur Tokens, nie sein HTML-Renderer; außerhalb des Haupteinstiegs |
| `react-day-picker` | MIT | Das Kalenderraster und die Datumslogik für `Calendar`, `DatePicker` und `DateRangePicker` |
| `shiki` | MIT | Syntaxhervorhebung in `CodeBlock`, verzögert geladen mit expliziten Grammatiken |
| `uqr` | MIT | QR-Codes für `QRCode`, in der Einrichtung der Zwei-Faktor-Authentifizierung |

Peer-Abhängigkeiten: `react` und `react-dom` (MIT) sowie `@hugeicons/react` und `@hugeicons/core-free-icons` (MIT), das einzige Icon-Set.

### Schriften

| Schrift | Lizenz | Ort |
| --- | --- | --- |
| Inter (variabel, Subsets latin und latin-ext) | SIL OFL 1.1, kein reservierter Schriftname | `packages/ui/src/styles/fonts/`, aus dem Build von Fontsource; ausgeliefert in `dist/fonts/` mit `LICENSE-Inter.txt` |
| Geist Mono (variabel, Subsets latin und latin-ext) | SIL OFL 1.1, kein reservierter Schriftname | `packages/ui/src/styles/fonts/`, aus dem Build von Fontsource; ausgeliefert in `dist/fonts/` mit `LICENSE-GeistMono.txt` |

Die Schriftdateien sind unveränderte Subsets und behalten ihre Namen. `pnpm release:check` lehnt ein Paket ab, dem eine der beiden Lizenzdateien fehlt.

### Die Konsole

| Paket | Lizenz | Warum |
| --- | --- | --- |
| `@logicplanes/kaeso-ui`, `@hugeicons/react`, `@hugeicons/core-free-icons` | MIT | Jede Komponente und jedes Icon, aus `packages/ui` |
| `react`, `react-dom` | MIT | Die Oberfläche |
| `@tanstack/react-router` | MIT | Dateibasierte Routen mit typisierten Suchparametern, jede Route in eigene Chunks aufgeteilt |
| `@tanstack/react-query` | MIT | Serverzustand: Caching, erneutes Laden und Änderungen über die Platform API |
| `openapi-fetch` | MIT | Der typisierte Client der Platform API, über den aus `openapi.json` generierten Typen |
| `marked` | MIT | Der Markdown-Lexer hinter der Unterhaltung einer protokollierten Anfrage: nur Tokens, gezeichnet mit Komponenten der Bibliothek |

### Die Dokumentations-Website

| Paket | Lizenz | Warum |
| --- | --- | --- |
| `@logicplanes/kaeso-ui`, `@logicplanes/kaeso-tokens`, `@hugeicons/react`, `@hugeicons/core-free-icons` | MIT | Jede Komponente und jedes Icon, aus `packages/ui` und `packages/tokens` |
| `react`, `react-dom` | MIT | Die Oberfläche |
| `@tanstack/react-router` | MIT | Ein Routenbaum pro Sprache; jede Adresse lädt den Chunk ihrer Seite |
| `marked` (nur Build) | MIT | Der Markdown-Lexer des Content-Builds; Seiten werden zur Build-Zeit kompiliert, sodass er nie in den Browser gelangt |
| `image-size` (nur Build) | MIT | Breite und Höhe jedes Bildes, damit Seiten beim Laden der Bilder nicht verspringen |
| `mermaid` (nur Build) | MIT | Zeichnet die Diagramme der Seiten beim Build in Headless-Chromium als SVG-Dateien; nichts von seinem Code erreicht den Browser |

### Build-, Test- und Showcase-Werkzeuge

Keines davon ist Teil eines veröffentlichten Pakets oder des Builds der Konsole oder der Dokumentation.

| Paket | Lizenz | Warum |
| --- | --- | --- |
| `vite`, `@vitejs/plugin-react` | MIT | Builds, mit einem Einstiegspunkt pro Komponente in der Bibliothek |
| `typescript`, `tsx` | Apache-2.0, MIT | Typen, Deklarationen und der Build der Tokens |
| `tailwindcss`, `@tailwindcss/vite` | MIT | Die Styling-Engine; das Theme wird aus den Tokens generiert |
| `@tailwindcss/node`, `@tailwindcss/oxide` | MIT | Compiler und Klassen-Scanner hinter der `styles.css` der Bibliothek, für Apps ohne Tailwind CSS |
| `react-docgen-typescript` | MIT | Die Komponenten-Registry und die Props-Tabellen |
| `@tanstack/router-plugin`, `@tanstack/router-generator` | MIT | Der Routenbaum der Konsole, aus `src/routes` generiert, und seine Aufteilung in Chunks |
| `openapi-typescript`, `@redocly/openapi-core` | MIT | Die Typen der Platform API für die Konsole, aus `openapi.json` generiert |
| `openai`, `@anthropic-ai/sdk` | Apache-2.0, MIT | Beispielverkehr durch das Gateway in `pnpm seed`; `@anthropic-ai/sdk` fordert außerdem die Release-Notizen in `scripts/release-notes.mjs` an ([geschriebene Release-Notizen](releasing.md#geschriebene-release-notizen)) |
| `vitest`, `@vitest/coverage-v8`, `happy-dom` | MIT | Unit- und Komponententests mit Coverage |
| `@testing-library/react`, `@testing-library/user-event`, `@testing-library/jest-dom` | MIT | Tests aus Sicht dessen, was eine Person sieht |
| `@playwright/test` | Apache-2.0 | End-to-End-Tests und das Headless-Chromium, das die Diagramme der Dokumentation zeichnet |
| `@axe-core/playwright`, `axe-core` | MPL-2.0 | Barrierefreiheitsprüfungen in den End-to-End-Tests |
| `pngjs` | MIT | Liest Screenshots in den Geometrieprüfungen des Showcases |
| `astro`, `@astrojs/react`, `@astrojs/mdx` | MIT | Der Showcase |
| `eslint`, `@eslint/js`, `typescript-eslint`, `eslint-plugin-react-hooks`, `eslint-plugin-jsx-a11y`, `globals` | MIT | Linting für die Konsole und die Dokumentations-Website, mit ihren eigenen Regeln in `eslint/` |
| `oxlint`, `prettier`, `prettier-plugin-astro`, `knip` | MIT, MIT, MIT, ISC | Linting für die Bibliotheken und den Showcase, Formatierung und Prüfungen auf ungenutzten Code |
| `@types/*` | MIT | Typdefinitionen |

`axe-core` steht unter MPL-2.0, einer Copyleft-Lizenz auf Dateiebene; es läuft nur in Tests und wird nie gebündelt. Der Build des Showcases zieht über Astro weitere Werkzeuge nach, darunter `sharp` mit libvips unter LGPL-3.0 für die Bildverarbeitung und `lightningcss` unter MPL-2.0; sie laufen zur Build-Zeit, und nichts von ihrem Code gelangt in ein Paket oder eine veröffentlichte Website.

## Python-Testwerkzeuge

Die End-to-End-Tests und die SDK-Konformitätssuite von KPP installieren die gepinnten Pakete aus `e2e/requirements.txt` in eine virtuelle Umgebung in `e2e/.venv`. Sie gelangen nie in ein Image oder ein Binary.

| Paket | Version | Warum | Lizenz |
| --- | --- | --- | --- |
| `openai` | 3.29.0 | Das offizielle OpenAI-SDK, das getestet wird | Apache-2.0 |
| `anthropic` | 1.13.0 | Das offizielle Anthropic-SDK, das getestet wird | MIT |
| `httpx` | 0.28.1 | Aufrufe der Platform API beim Einrichten der Suiten | BSD-3-Clause |
| `pytest` | 9.1.1 | Der Test-Runner | MIT |
