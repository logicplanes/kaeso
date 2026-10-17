---
title: Tests
description: Jede Testsuite im Repository, wie man sie ausführt und was bestehen muss, bevor eine Änderung fertig ist.
order: 4
---

Jede Änderung kommt mit Tests, und Tests prüfen, was eine Person oder eine App sieht. Coverage-Schwellen werden nie gesenkt; steigt die Coverage, steigt die Schwelle mit. Jeder Befehl unten läuft vom Wurzelverzeichnis des Repositorys aus, bei Suiten, die Datenbanken brauchen, mit laufendem [Entwicklungs-Stack](local-setup.md#3-entwicklungs-stack-starten).

## Bevor eine Änderung fertig ist

Der TypeScript-Workspace:

```bash
pnpm lint
pnpm format:check
pnpm typecheck
pnpm test
pnpm i18n
pnpm knip
pnpm build
pnpm licenses:check
pnpm release:check
pnpm e2e
```

Der Rust-Workspace:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo coverage-kpp
cargo coverage-gateway
cargo test -p xtask -p kpp-conformance
cargo deny check
cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check
cargo sqlx prepare --workspace --check
cargo run -q -p kaeso -- openapi | diff - openapi.json
```

Dann die Suiten der geänderten Teile ausführen: die [End-to-End-Tests](#end-to-end-tests) für das Gateway, KPP oder das Packaging, `docker build -f deploy/console.Dockerfile .` für das Packaging und den [Fuzz-Build](#fuzzing), die [Benchmarks](performance.md#kpp) und `cargo test --doc -p kaeso-kpp -p kaeso-codecs` für KPP. Jede geänderte Oberfläche hell und dunkel, in Desktop- und Telefonbreite und auf Englisch, Deutsch und Chinesisch ansehen. Die [Continuous Integration](#continuous-integration) führt dieselben Prüfungen bei jedem Pull Request aus.

## TypeScript

| Befehl | Was er prüft |
| --- | --- |
| `pnpm lint` | ESLint oder oxlint in jedem Paket, mit Kaesos eigenen Regeln: Klassennamen nur für Layout, keine Farben außerhalb der Tokens, kein direkt in die Oberfläche der Konsole geschriebener Text; für die Dokumentation jede Seite mit `check:content` |
| `pnpm format:check` | Prettier |
| `pnpm typecheck` | TypeScript, nach dem Build der Bibliotheken in `packages/` |
| `pnpm test` | Vitest mit Coverage-Schwellen, in beiden Bibliotheken, der Konsole und der Dokumentation, danach die Tests der Skripte des Repositorys in `scripts/`, etwa die des Changelogs, mit `node --test`; die Tests der Versionsprüfung prüfen auch das Repository selbst ([die Version im Repository](releasing.md#die-version-im-repository)) |
| `pnpm i18n` | Jeden Text der Konsole auf Deutsch und Chinesisch, mit denselben Platzhaltern und ohne Überbleibsel ([Übersetzungen](translations.md#prüfen)) |
| `pnpm knip` | Ungenutzte Dateien, Exporte und Abhängigkeiten |
| `pnpm build` | Jeden Produktions-Build, am Ende mit seinen Bundle-Budgets ([Performance](performance.md)); die Builds beider Websites führen danach die Prüfung der Website aus ([die Websites](releasing.md#die-websites)) |
| `pnpm licenses:check` | Nach einem Build: die Lizenz jedes Pakets, dessen Code ausgeliefert wird ([Abhängigkeiten](dependencies.md#lizenzrichtlinie)) |
| `pnpm release:check` | Nach einem Build: packt beide Bibliotheken wie npm und prüft, dass jede Datei, die sie ausliefern müssen, enthalten ist, in einer Version |
| `pnpm e2e` | Playwright in der Konsole, der Dokumentation und dem Showcase, nacheinander; die Suite des Showcase läuft gegen seinen Build, daher zuerst `pnpm build` |

`pnpm --filter <package> <script>` führt das Skript eines einzelnen Pakets aus; die Pakete sind `kaeso-console`, `kaeso-docs`, `kaeso-showcase`, `@logicplanes/kaeso-ui` und `@logicplanes/kaeso-tokens`. `pnpm typecheck`, `pnpm test` und `pnpm knip` bauen zuerst die Bibliotheken; haben sie sich geändert, vor einem gefilterten Lauf `pnpm build:packages` ausführen.

Jedes Paket mit Tests legt in seiner `vitest.config.ts` Schwellen für Zeilen, Anweisungen, Zweige und Funktionen fest: 100 % für die Tokens, für die anderen ihre gemessene Coverage, auf ein Zehntelprozent abgerundet.

Playwright braucht Chromium, einmal installiert:

```bash
pnpm --filter kaeso-console exec playwright install chromium
```

### Die Konsole

Unit-Tests laufen in happy-dom gegen eine pro Test gestubbte Platform API (`apps/console/src/test/network.ts`, mit Fixtures in `apps/console/src/test/fixtures/`). `renderApp(path)` mountet die ganze Konsole unter einer Adresse mit eigenem Router, der mit dem Test endet, sodass nichts, wohin ein Test navigiert, den nächsten erreicht; `renderUi` mountet eine einzelne Komponente. Tests prüfen, was eine Person sieht und welche Anfragen gesendet werden, und bestehen in jeder Reihenfolge, was `pnpm --filter kaeso-console exec vitest run --sequence.shuffle` prüft. Testdateien unter `src/routes` beginnen mit `-`, damit der Routengenerator sie überspringt.

Die End-to-End-Suite berührt nie den Entwicklungs-Stack. Sie startet einen eigenen Stack aus `deploy/compose.dev.yaml`, das Compose-Projekt `kaeso-console-e2e` auf den 28000er-Ports mit dem Gateway auf `127.0.0.1:28080`, liefert die Konsole mit Vite unter http://localhost:3100 aus und entfernt den Stack samt Daten, wenn der Lauf endet. `KAESO_GATEWAY_IMAGE` nutzt ein bereits gebautes Gateway-Image, statt eines aus dem Checkout zu bauen. Die Specs bauen aufeinander auf, beginnend mit `01-public`, das die Installation einrichtet. Um einen End-to-End-Stack zwischen Läufen am Laufen zu halten, ihn von Hand starten und es der Suite mitteilen:

```bash
node apps/console/scripts/stack.mjs up --e2e
KAESO_E2E_STACK=running pnpm --filter kaeso-console e2e
```

Ein Test schlägt bei jedem Fehler fehl, den der Browser während seines Laufs meldet: ein nicht abgefangener Fehler oder eine nicht behandelte Rejection oder ein `console.error`, über den React und Base UI warnen. Specs importieren `test` und `expect` aus `apps/console/e2e/support.ts`, wo diese Prüfung liegt.

### Die Dokumentations-Website

Die Unit-Tests und die End-to-End-Suite nutzen die Fixture-Seiten in `apps/docs/e2e/fixtures/content`, nie die echten Seiten, sodass eine Änderung an der Dokumentation nie einen Test bricht. `pnpm --filter kaeso-docs e2e` führt Playwright samt axe gegen einen Produktions-Build dieser Seiten aus. Die echten Seiten prüfen `pnpm --filter kaeso-docs check:content` und der Build. Die Suite führt außerdem die Prüfung der Website auf ihrem Build aus und öffnet die Suche aus der Leiste für Telefone bei 375 mal 812 Pixeln in jeder Sprache; die Tests der Prüfung selbst stehen in `scripts/check-site.test.mjs`.

### Die UI-Bibliothek und der Showcase

Komponententests laufen in happy-dom mit Testing Library und prüfen, was eine Person sieht. Der Showcase hat keine Unit-Tests; seine End-to-End-Suite führt Playwright gegen seinen Produktions-Build aus, daher zuerst `pnpm build`: axe im hellen und dunklen Theme, Tastatur und Fokusringe, Schriftskala, Bewegung und Geometrie gegen die Tokens, Menüs, die an Ort und Stelle öffnen, jede Komponentenseite in Telefonbreite ohne seitliches Scrollen die deutschen und chinesischen Seiten, die schon vor jedem Skript in ihrer Sprache geschrieben sind und ohne Abweichung hydrieren, die Suche, geöffnet aus der Kopfleiste für Telefone bei 375 mal 812 Pixeln in jeder Sprache, die Sitemap und den Kopf jeder Art von Seite sowie Links ins Repository, die den Tag des Releases nennen.

## Rust

Die Cargo-Werkzeuge einmal installieren:

```bash
cargo install cargo-deny cargo-llvm-cov sqlx-cli --locked
```

| Befehl | Was er prüft |
| --- | --- |
| `cargo fmt --all --check` | Formatierung (`rustfmt.toml`) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Lints, darunter kein `unwrap`, `expect`, `panic!` oder panikfähige Indizierung außerhalb von Tests (`Cargo.toml`, `clippy.toml`) |
| `cargo test --workspace --all-features` | Jeden Test |
| `cargo coverage-kpp` | Die Tests von `kaeso-kpp` und `kaeso-codecs` unter `cargo llvm-cov`, schlägt unter 95 % der Zeilen fehl |
| `cargo test --doc -p kaeso-kpp -p kaeso-codecs` | Die Beispiele in den READMEs von `kaeso-kpp` und `kaeso-codecs`, der Dokumentation der Crates auf docs.rs, die `cargo llvm-cov` nicht ausführt |
| `cargo coverage-gateway` | Die Tests aller anderen Crates außer `kpp-conformance` und `xtask` unter `cargo llvm-cov`, schlägt unter 91 % der Zeilen fehl |
| `cargo test -p xtask -p kpp-conformance` | Die Repository-Aufgaben und den Konformitätsserver, die die Coverage-Befehle auslassen |
| `cargo deny check` | Lizenzen, Advisories, Verbote und Quellen jeder Abhängigkeit (`deny.toml`) |
| `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` | Die strengere Richtlinie von KPP (`crates/kaeso-codecs/deny.toml`): keine Runtime- oder HTTP-Crates in der Bibliothek |
| `cargo sqlx prepare --workspace --check` | Die Offline-Abfragedaten in `.sqlx/` passen zu den Abfragen |
| `cargo run -q -p kaeso -- openapi \| diff - openapi.json` | `openapi.json` ist aktuell |

Die Coverage-Befehle sind Aliasse in `.cargo/config.toml`, das auch auflistet, was sie auslassen, etwa Testhilfen und das `main` des Binarys. Tests, die die Datenbanken brauchen, schlagen fehl, statt übersprungen zu werden, wenn die Datenbanken nicht da sind.

`kaeso-kpp` und `kaeso-codecs` nennen die älteste Rust-Version, mit der sie bauen, als `rust-version` in ihrer `Cargo.toml`, und die CI prüft und testet jedes Crate mit dieser Version. Um dasselbe zu tun, diese Version mit `rustup toolchain install` installieren und `cargo +<version> test --locked -p <crate>` ausführen.

`cargo sqlx prepare --check` braucht `DATABASE_URL=postgres://kaeso:kaeso@127.0.0.1:55432/kaeso` mit angewendeten Migrationen. Das Gateway des Stacks wendet sie beim Start an; ohne es `cargo sqlx migrate run --source crates/kaeso-store/migrations/postgres` ausführen.

### Testdatenbanken

Jeder Test legt eigene Datenbanken auf dem PostgreSQL, ClickHouse und Valkey der Entwicklungsumgebung an, sodass Tests parallel laufen, ohne Zustand zu teilen. `kaeso-testkit` benennt sie und räumt sie auf:

| Name | Angelegt von | Gelöscht |
| --- | --- | --- |
| `t_<uuid>` | `TestServices::start` in einem Test oder `kaeso-testkit fresh-services --temporary` | Wenn der Test endet, auch bei einem Panic; sonst durch die Bereinigung |
| `dev_<uuid>` | `kaeso-testkit fresh-services`, für ein Gateway, das [aus dem Quellcode läuft](local-setup.md#das-gateway-aus-dem-quellcode-ausführen) | Nur auf Anforderung: `kaeso-testkit drop-services dev_<uuid>` |

Der erste Test in einem Prozess löscht `t_`-Datenbanken, die älter als sechs Stunden sind und nicht mehr genutzt werden. `KAESO_TEST_KEEP_DATABASES=1 cargo test …` behält die Datenbanken eines fehlgeschlagenen Tests und gibt ihre Namen aus. `KAESO_TEST_POSTGRES_URL`, `KAESO_TEST_CLICKHOUSE_URL` und `KAESO_TEST_VALKEY_URL` richten die Tests auf andere Server.

### Snapshots und Referenzen

Manche Tests vergleichen mit `insta` gegen geprüfte Snapshots. Ändert sich einer, den Diff prüfen und aktualisieren, was davon abhängt:

| Snapshot | Enthält | Ebenfalls aktualisieren |
| --- | --- | --- |
| `crates/kaeso-core/src/rights/snapshots/` | Jedes Recht und jede eingebaute Rolle | [Rechte](../concepts/rights.md) |
| `crates/kaeso-server/tests/snapshots/` | Jede Einstellung mit ihrem Standardwert und ihrer Beschreibung | [Konfiguration](../self-hosting/configuration.md) und `deploy/kaeso.example.toml` |
| `crates/kaeso-codecs/tests/snapshots/` | Die Ergebnisse der dialektübergreifenden Matrix | Die Mapping-Seite des Dialekts ([KPP-Interna](kpp/index.md#tests)) |
| `crates/kaeso-mail/src/snapshots/` | Text und HTML jeder E-Mail | |

`crates/kaeso-server/tests/error_copy.rs` prüft jede Meldung, die die Platform API senden kann, gegen die [Regeln für Fehlertexte](architecture/gateway.md#fehlermeldungen), und `crates/kaeso-server/tests/docs_paths.rs` prüft, dass jeder Repository-Pfad und jeder relative Link in den Markdown-Dateien im Wurzelverzeichnis und in `crates/`, `deploy/`, `e2e/`, `fuzz/` und `.github/` auf etwas Vorhandenes zeigt.

## End-to-End-Tests

Die Suiten in `e2e/` lassen die offiziellen Python-SDKs von OpenAI und Anthropic, festgelegt in `e2e/requirements.txt`, gegen die Encoder von KPP, das Gateway und eine frische Installation laufen. Ihre virtuelle Umgebung einmal einrichten:

```bash
python3 -m venv e2e/.venv
e2e/.venv/bin/pip install -r e2e/requirements.txt
```

### SDK-Konformität

```bash
e2e/.venv/bin/pytest -q e2e/conformance
```

Prüft, dass die SDKs alles akzeptieren, was die Encoder von KPP erzeugen. Die Suite baut und startet `kpp-conformance` (`crates/kpp-conformance`, ein Release-Build auf einem zufälligen Port); mit `KPP_CONFORMANCE_URL=http://127.0.0.1:8787` läuft sie stattdessen gegen einen von Hand mit `cargo run --release -p kpp-conformance` gestarteten Server.

```text
SDK request ──▶ server: decode with the endpoint's codec (ingress decode)
                 model = "<source dialect>:<fixture>"
                 fixture ──▶ decode with the SOURCE codec (egress decode) ──▶ KPP
                 KPP ──▶ encode with the endpoint's codec (ingress encode)
SDK parses ◀── response / SSE stream / error
```

| Endpunkt | Codec | SDK-Aufrufe |
| --- | --- | --- |
| `POST /v1/messages` | `anthropic_messages` | `messages.create`, `messages.stream` |
| `POST /v1/messages/count_tokens` | `anthropic_messages` | `messages.count_tokens` |
| `POST /v1/chat/completions` | `openai_chat` | `chat.completions.create`, `chat.completions.stream` |
| `POST /v1/responses` | `openai_responses` | `responses.create`, `responses.stream` |
| `POST /v1/responses/input_tokens` | `openai_responses` | `responses.input_tokens.count` |
| `POST /v1/embeddings` | `openai_embeddings` | `embeddings.create` |
| `GET /v1/models`, `GET /v1/models/{id}` | `anthropic_messages` mit einem Header `anthropic-version`, sonst `openai_chat` | `models.list`, `models.retrieve` |

Jede Antwort- und Stream-[Fixture](kpp/fixtures.md) jedes Generierungsdialekts wird über jeden Generierungsendpunkt ausgeliefert, im selben Dialekt und dialektübergreifend, gestreamt und nicht gestreamt. Die Tests vergleichen, was das SDK geparst hat (Text, Tool-Aufrufe), mit dem, was KPP dekodiert hat (`GET /_expected`), sodass eine Übersetzung, die Text, Tool-Argumente oder lesbares Reasoning verliert, fehlschlägt:

- das Reasoning, das das SDK bereitstellt (Zusammenfassungen und Text der Reasoning-Items bei Responses, Text der `thinking`-Blöcke bei Anthropic), muss dem dekodierten Reasoning entsprechen, und in Responses-Streams müssen sich die Events `response.reasoning_summary_text.delta` zu jedem `response.reasoning_summary_text.done` und zu den Zusammenfassungen der finalen Antwort zusammensetzen;
- Antworten, deren Fixture strukturierte Ausgabe ist, müssen gültiges JSON bleiben, wenn das SDK sie liest;
- jede Fehler-Fixture jedes Dialekts, über die Endpunkte für Anthropic, Chat und Responses ausgeliefert, muss im SDK einen `APIStatusError` mit dem Status auslösen, der in diesem Format zu ihrer KPP-Fehlerart gehört, bei Ratenlimits `retry-after` behalten und den Fehler-Body des Formats verwenden (`e2e/conformance/test_errors.py`);
- die Modell-Endpunkte liefern einen festen Katalog aus fünf Modellen, eines davon ausgemustert, mit `ModelList::page` seitenweise: `e2e/conformance/test_models.py` folgt jeder Seite mit dem Auto-Paging des Anthropic-SDK vorwärts (`limit=2`) und rückwärts (`before_id`), filtert nach `lifecycle`, erwartet `BadRequestError` für einen unbekannten Cursor oder `limit=0`, listet mit dem OpenAI-SDK, ruft Modelle ab und erwartet in beiden SDKs `NotFoundError` für unbekannte IDs.

`GET /_scenarios` listet jedes Szenario, das der Server anbietet.

### SDK-Suite

```bash
e2e/.venv/bin/pytest -q e2e/sdk
```

Braucht die Entwicklungsdatenbanken. Sie baut das Gateway und `kaeso-testkit`, legt frische Datenbanken an, startet den Mock-Anbieter und das Gateway und richtet über die Platform API den ersten Benutzer, eine Organisation und ein Projekt, Anbieter-Schlüssel, Katalogmodelle, einen Alias und API-Schlüssel ein. Die Tests decken Streaming, Keep-alives über einen stillen Anbieter, Tools, Bilder, PDFs, Reasoning, beide Richtungen zwischen Anbietern, Token-Zählung, Embeddings, Modelllisten, jede Fehlerklasse, Limits, Wiederholungen des SDK und Verbindungsabbrüche ab.

Der Mock-Anbieter wählt seine Antwort anhand von Markierungen in der Anfrage (`crates/kaeso-testkit/src/mock_provider.rs`); `[mock:fixture=NAME]` nennt eine Fixture im Format des Anbieters, und `[mock:gate=NAME]` hält die Anfrage fest, bis der Test das Tor öffnet. `MockProvider::gate()` erzeugt ein Tor und liefert dessen Markierung für den Prompt; `wait_for(n)` kehrt zurück, sobald `n` Anfragen es erreicht haben, und `open()` lässt alle durch. So hält ein Test eine Anfrage ohne Sleep in der Schwebe.

### Installationstest

```bash
e2e/.venv/bin/pytest -q e2e/install
```

Baut die Images des Gateways und des Mock-Anbieters aus `deploy/gateway.Dockerfile`, betreibt `deploy/compose.yaml` von einem sauberen Zustand aus mit den Overrides in `e2e/install`, schließt die Einrichtung beim ersten Start ab, ruft die Installation mit beiden SDKs auf und prüft, dass PostgreSQL und ClickHouse dem Gateway keine Rechte geben, die es nicht braucht, und dass Valkey Verbindungen ohne sein Passwort ablehnt. Ein Platzhalter bedient die Adresse der Konsole; `KAESO_TEST_CONSOLE_IMAGE` nutzt stattdessen das echte Konsolen-Image, und `KAESO_TEST_GATEWAY_IMAGE` und `KAESO_TEST_MOCK_IMAGE` nutzen bereits gebaute Images. Immer ausführen, wenn sich das Packaging, ein Dockerfile oder `deploy/compose.yaml` ändert.

## Fuzzing

`fuzz/` enthält ein [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html)-Target pro KPP-Decoder, dazu den SSE-Parser. Es ist ein eigener Cargo-Workspace, aus dem Workspace des Repositorys ausgenommen, wie cargo-fuzz es erwartet, sodass `libfuzzer-sys` nie die Bibliotheks-Crates erreicht.

| Target | Prüft |
| --- | --- |
| `decode_anthropic_messages`, `decode_openai_chat`, `decode_openai_responses`, `decode_openai_embeddings` | Jeder Decoder des Dialekts (Anfragen aller drei Arten, Antworten, Fehler, Modelllisten, Streams, in Stücken eingespeist, deren Größe die Eingabe bestimmt) akzeptiert beliebige Bytes ohne Panic, und auch das Kodieren des Dekodierten in jeden Dialekt löst keinen Panic aus |
| `sse` | Der SSE-Parser löst nie einen Panic aus und liefert dieselben Items, wie auch immer die Eingabe aufgeteilt ist, und die geparsten Items, wieder geschrieben und erneut geparst, ergeben dieselben Items |

Die gemeinsame Logik liegt in `fuzz/src/lib.rs`; jede Datei in `fuzz/fuzz_targets/` nennt ihren Dialekt. Jede Änderung prüft, dass die Targets bauen und formatiert und gelintet sind:

```bash
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo fmt --all --check --manifest-path fuzz/Cargo.toml
cargo clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings
```

Zum Ausführen braucht es eine Nightly-Toolchain und cargo-fuzz:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

Ein Target mit einem Korpus-Ordner und den Fixtures des Dialekts als Seed-Korpus ausführen; das Target `sse` nimmt jede Fixture:

```bash
mkdir -p fuzz/corpus/decode_openai_chat
cargo +nightly fuzz run decode_openai_chat fuzz/corpus/decode_openai_chat crates/kaeso-codecs/fixtures/openai_chat -- -max_total_time=600 -rss_limit_mb=2048 -max_len=65536
```

```bash
mkdir -p fuzz/corpus/sse
cargo +nightly fuzz run sse fuzz/corpus/sse crates/kaeso-codecs/fixtures -- -max_total_time=600
```

Ein Absturz wird nach `fuzz/artifacts/<target>/` geschrieben. Ihn mit `cargo +nightly fuzz run <target> <file>` reproduzieren, daraus einen Regressionstest im Codec machen und dann den Codec korrigieren. [Performance](performance.md#fuzzing) hält den letzten vollständigen Lauf fest.

## Continuous Integration

Der CI-Workflow (`.github/workflows/ci.yml`) läuft bei jedem Pull Request, bei jedem Push auf `dev` und `prod`, in der Merge-Queue und aus dem Release-Workflow. Die eigenen Pushes des Release-Workflows, sein Release-Commit und das nach `dev` gebrachte Release, starten ihn nicht erneut: Das Release hat jeden Job schon bestanden. Bei einem Pull Request bestimmt sein erster Job, welche Bereiche sich geändert haben, und die Jobs unveränderter Bereiche werden übersprungen; sonst laufen alle Jobs. Die einzige erforderliche Prüfung ist **CI**: Sie besteht, wenn **Changes** und **No comments** bestanden haben und jeder andere Job bestanden hat oder übersprungen wurde.

| Job | Führt aus |
| --- | --- |
| Changes | Bestimmt, welche Bereiche ein Pull Request ändert: Rust, TypeScript, das Deployment, die eigenen Dateien des Repositorys, die Workflows |
| No comments | Keine Kommentare in Quelldateien, Konfiguration oder Codeblöcken in Markdown |
| Workflows | `actionlint` |
| Rust format | `cargo fmt --all --check`, für den Workspace und `fuzz/` |
| Rust lint | `cargo clippy` mit jeder Warnung als Fehler, für den Workspace und `fuzz/` |
| Test KPP | `cargo coverage-kpp` und `cargo test --doc -p kaeso-kpp -p kaeso-codecs` |
| Minimum Rust version | `cargo check` und `cargo test` von `kaeso-kpp` und von `kaeso-codecs`, jeweils mit der Rust-Version aus seiner `rust-version` |
| Test gateway | `cargo coverage-gateway`, mit dem Entwicklungs-Stack |
| Repository tooling | `node scripts/version.mjs check`, `cargo test -p xtask -p kpp-conformance` und `cargo test -p kaeso-server --test docs_paths` |
| Query data | Die PostgreSQL-Migrationen, dann `cargo sqlx prepare --workspace --check` |
| OpenAPI document | `cargo run -q -p kaeso -- openapi \| diff - openapi.json` |
| Rust dependencies | `cargo deny check`, mit der Richtlinie des Workspace und der von KPP |
| Fuzz targets | `cargo check --manifest-path fuzz/Cargo.toml --bins` |
| Benchmarks build | `cargo bench -p kaeso-codecs --bench codecs --no-run` |
| SDK conformance | `pytest -q e2e/conformance` |
| SDK end-to-end | `pytest -q e2e/sdk`, mit dem Entwicklungs-Stack |
| Compose config | `docker compose config` für `deploy/compose.yaml` und `deploy/compose.dev.yaml` |
| Images | Baut die Images des Gateways, des Mock-Anbieters und der Konsole, beschriftet mit den Versionen aus `Cargo.toml` und `apps/console/package.json`, und prüft, dass das Konsolen-Image auf `/healthz` antwortet, die Konsole mit einer Content Security Policy ausliefert und für eine fehlende Datei unter `/assets/` mit 404 antwortet |
| Install test | `pytest -q e2e/install` mit diesen Images, das echte Konsolen-Image eingeschlossen |
| TypeScript lint | `pnpm format:check`, `pnpm lint`, `pnpm i18n` und `pnpm knip` |
| TypeScript typecheck | `pnpm typecheck` |
| TypeScript test | `pnpm test`, mit der Coverage jedes Pakets in der Zusammenfassung des Jobs |
| TypeScript build | `pnpm build`, `pnpm licenses:check` und `pnpm release:check` |
| End-to-end console | `pnpm --filter kaeso-console e2e` gegen das Gateway-Image |
| End-to-end docs | `pnpm --filter kaeso-docs e2e` |
| End-to-end showcase | `pnpm --filter kaeso-showcase e2e` gegen den Showcase, den TypeScript build gebaut hat |
| TypeScript dependencies | `pnpm audit --prod --audit-level high` |
| CI | Die erforderliche Prüfung |
