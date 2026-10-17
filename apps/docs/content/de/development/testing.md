---
title: Tests
description: Jede Testsuite im Repository, wie man sie ausführt und was bestehen muss, bevor eine Änderung fertig ist.
order: 4
---

Jede Änderung kommt mit Tests, die prüfen, was eine Person oder eine App sieht. Coverage-Schwellen werden nie gesenkt; steigt die Coverage, steigt die Schwelle mit. Befehle laufen vom Wurzelverzeichnis des Repositorys aus, für Suites mit Datenbanken bei laufendem [Entwicklungs-Stack](local-setup.md#3-entwicklungs-stack-starten).

## Bevor eine Änderung fertig ist

Diese Befehle bestehen bei jeder Änderung:

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

Dazu, je nach geändertem Teil:

| Geändert | Zusätzlich ausführen |
| --- | --- |
| KPP | Die [SDK-Konformität](#sdk-konformität), `cargo test --doc -p kaeso-kpp -p kaeso-codecs`, die [Prüfungen der Fuzz-Targets](#fuzzing) und `cargo bench -p kaeso-codecs --bench codecs --no-run` |
| Das Gateway | Die [SDK-Suite](#sdk-suite) |
| `deploy/`, ein Dockerfile, das Packaging | Den [Installationstest](#installationstest) und `docker build -f deploy/console.Dockerfile .` |
| `.github/workflows` | `actionlint` |
| Eine Oberfläche | Jede geänderte Ansicht in Hell und Dunkel, in Desktop- und Telefonbreite, auf Englisch, Deutsch und Chinesisch ansehen |

Die [Continuous Integration](#continuous-integration) führt dieselben Prüfungen für jeden Pull Request aus.

## TypeScript

| Befehl | Prüft |
| --- | --- |
| `pnpm lint` | ESLint oder oxlint in jedem Paket, mit Kaesos eigenen Regeln: Klassennamen nur für Layout, keine Farben außerhalb der Tokens, kein direkt in die Konsole geschriebener Text; für die Dokumentation jede Seite mit `check:content` |
| `pnpm format:check` | Prettier |
| `pnpm typecheck` | TypeScript, nachdem die Bibliotheken in `packages/` gebaut sind |
| `pnpm test` | Vitest mit Coverage-Schwellen in beiden Bibliotheken, der Konsole und der Dokumentation, danach die Tests der Skripte in `scripts/` mit `node --test`, darunter die [Versionsprüfung](releasing.md#die-version-im-repository) am Repository selbst |
| `pnpm i18n` | Jeder Text der Konsole hat Deutsch und Chinesisch mit denselben Platzhaltern, und nichts ist übrig ([Übersetzungen](translations.md#prüfen)) |
| `pnpm knip` | Ungenutzte Dateien, Exporte und Abhängigkeiten |
| `pnpm build` | Jeden Produktions-Build und seine Bundle-Budgets ([Performance](performance.md)); die Builds beider Websites führen danach die [Website-Prüfung](releasing.md#die-websites) aus |
| `pnpm licenses:check` | Nach einem Build: die Lizenz jedes Pakets, dessen Code ausgeliefert wird ([Abhängigkeiten](dependencies.md#lizenzrichtlinie)) |
| `pnpm release:check` | Nach einem Build: packt beide Bibliotheken wie npm und prüft, dass jede Datei, die sie ausliefern müssen, in einer Version vorhanden ist |
| `pnpm e2e` | Playwright in Konsole, Dokumentation und Showcase, nacheinander; die Suite des Showcases testet dessen Build, deshalb kommt `pnpm build` zuerst |

`pnpm --filter <package> <script>` führt das Skript eines Pakets aus; die Pakete heißen `kaeso-console`, `kaeso-docs`, `kaeso-showcase`, `@logicplanes/kaeso-ui` und `@logicplanes/kaeso-tokens`. `pnpm typecheck`, `pnpm test` und `pnpm knip` bauen zuerst die Bibliotheken; vor einem gefilterten Lauf `pnpm build:packages` ausführen, wenn sich die Bibliotheken geändert haben.

Jedes Paket mit Tests legt in seiner `vitest.config.ts` Schwellen für Zeilen, Anweisungen, Zweige und Funktionen fest: 100 % für die Tokens, für die anderen ihre gemessene Coverage, auf ein Zehntelprozent abgerundet.

Playwright braucht Chromium, einmal installiert:

```bash
pnpm --filter kaeso-console exec playwright install chromium
```

### Die Konsole

Unit-Tests laufen in happy-dom gegen eine pro Test gestubbte Platform API (`apps/console/src/test/network.ts`, Fixtures in `apps/console/src/test/fixtures/`). `renderApp(path)` bindet die ganze Konsole unter einer Adresse mit eigenem Router ein, der mit dem Test endet; `renderUi` bindet eine einzelne Komponente ein. Tests prüfen, was eine Person sieht und welche Anfragen gesendet werden, und bestehen in jeder Reihenfolge, was `pnpm --filter kaeso-console exec vitest run --sequence.shuffle` prüft. Testdateien unter `src/routes` beginnen mit `-`, damit der Routengenerator sie überspringt.

Die End-to-End-Suite berührt den Entwicklungs-Stack nie. Sie startet einen eigenen aus `deploy/compose.dev.yaml`: das Compose-Projekt `kaeso-console-e2e`, auf Ports im Bereich 28000 mit dem Gateway auf `127.0.0.1:28080` und der Konsole unter http://localhost:3100. Für `20-sso-only` kommt ein zweites Gateway hinzu, das nur Single Sign-on erlaubt, auf `127.0.0.1:28081` mit seiner Konsole unter http://localhost:3101. Stack und Daten werden am Ende des Laufs entfernt. Die Specs bauen aufeinander auf, beginnend mit `01-public`, das die Installation einrichtet. `KAESO_GATEWAY_IMAGE` nutzt ein bereits gebautes Gateway-Image.

Um den End-to-End-Stack zwischen Läufen weiterlaufen zu lassen, ihn selbst starten und der Suite Bescheid geben:

```bash
node apps/console/scripts/stack.mjs up --e2e
KAESO_E2E_STACK=running pnpm --filter kaeso-console e2e
```

Ein Test schlägt bei jedem Fehler fehl, den der Browser während des Laufs meldet: einem nicht abgefangenen Fehler oder einer Rejection oder einem `console.error`, über den React und Base UI warnen. Specs importieren `test` und `expect` aus `apps/console/e2e/support.ts`, das diese Prüfung enthält.

### Die Dokumentations-Website

Unit-Tests und End-to-End-Suite nutzen die Fixture-Seiten in `apps/docs/e2e/fixtures/content`, nie die echten Seiten, sodass eine Änderung an der Dokumentation nie einen Test bricht. `pnpm --filter kaeso-docs e2e` führt Playwright samt axe gegen einen Produktions-Build der Fixtures aus; die Suite führt auch die Website-Prüfung an diesem Build aus und öffnet die Suche aus der Telefonleiste bei 375 × 812 Pixeln in jeder Sprache. Die echten Seiten prüfen `pnpm --filter kaeso-docs check:content` und der Build. Die Tests der Website-Prüfung selbst stehen in `scripts/check-site.test.mjs`. Die Tests des Diagramm-Renderers zeichnen in Chromium, wie der Build, daher braucht `pnpm test` das Chromium, das Playwright installiert.

### Die UI-Bibliothek und der Showcase

Komponententests laufen in happy-dom mit Testing Library. Der Showcase hat keine Unit-Tests; seine End-to-End-Suite führt Playwright gegen seinen Produktions-Build aus, deshalb kommt `pnpm build` zuerst. Sie prüft:

- axe in Hell und Dunkel, Tastatur und Fokusringe;
- Schriftskala, Bewegung und Geometrie gegen die Tokens und Menüs, die an Ort und Stelle aufgehen;
- jede Komponentenseite in Telefonbreite ohne seitliches Scrollen;
- die deutschen und chinesischen Seiten, schon vor jedem Skript in ihrer Sprache und ohne Abweichung hydriert;
- die Suche, aus der Telefonkopfzeile bei 375 × 812 Pixeln in jeder Sprache geöffnet;
- die Sitemap, den Head jeder Art von Seite und Links ins Repository, die den Tag des Releases nennen.

## Rust

Die Cargo-Werkzeuge einmal installieren:

```bash
cargo install cargo-deny cargo-llvm-cov sqlx-cli --locked
```

| Befehl | Prüft |
| --- | --- |
| `cargo fmt --all --check` | Formatierung (`rustfmt.toml`) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Lints, darunter kein `unwrap`, `expect`, `panic!` oder panikauslösendes Indexieren außerhalb von Tests (`Cargo.toml`, `clippy.toml`) |
| `cargo test --workspace --all-features` | Jeden Test |
| `cargo coverage-kpp` | Die Tests von `kaeso-kpp` und `kaeso-codecs` unter `cargo llvm-cov`; schlägt unter 95 % der Zeilen fehl |
| `cargo test --doc -p kaeso-kpp -p kaeso-codecs` | Die Beispiele in den READMEs beider Crates, die ihre Dokumentation auf docs.rs sind und die `cargo llvm-cov` nicht ausführt |
| `cargo coverage-gateway` | Die Tests aller anderen Crates außer `kpp-conformance` und `xtask` unter `cargo llvm-cov`; schlägt unter 91 % der Zeilen fehl |
| `cargo test -p xtask -p kpp-conformance` | Die Repository-Aufgaben und den Konformitätsserver, die die Coverage-Befehle auslassen |
| `cargo deny check` | Lizenzen, Sicherheitsmeldungen, Verbote und Quellen jeder Abhängigkeit (`deny.toml`) |
| `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` | Die strengere Richtlinie von KPP (`crates/kaeso-codecs/deny.toml`): keine Runtime- oder HTTP-Crates in der Bibliothek |
| `cargo sqlx prepare --workspace --check` | Die Offline-Abfragedaten in `.sqlx/` passen zu den Abfragen |
| `cargo run -q -p kaeso -- openapi \| diff - openapi.json` | `openapi.json` ist aktuell |

Die Coverage-Befehle sind Aliasse in `.cargo/config.toml`, die auch die ausgelassenen Dateien aufführt, etwa Testhelfer und die `main` des Binaries. Tests, die die Datenbanken brauchen, schlagen fehl statt übersprungen zu werden, wenn die Datenbanken fehlen.

`cargo sqlx prepare --check` braucht `DATABASE_URL=postgres://kaeso:kaeso@127.0.0.1:55432/kaeso` mit angewandten Migrationen. Das Gateway des Stacks wendet sie beim Start an; ohne es `cargo sqlx migrate run --source crates/kaeso-store/migrations/postgres` ausführen.

`kaeso-kpp` und `kaeso-codecs` geben das älteste unterstützte Rust als `rust-version` in ihrer `Cargo.toml` an, und CI prüft und testet jedes Crate damit. Um das selbst zu tun, die Version mit `rustup toolchain install <version>` installieren und `cargo +<version> test --locked -p <crate>` ausführen.

### Testdatenbanken

Jeder Test legt eigene Datenbanken auf PostgreSQL, ClickHouse und Valkey der Entwicklung an, sodass Tests ohne gemeinsamen Zustand parallel laufen. `kaeso-testkit` benennt und entfernt sie:

| Name | Angelegt von | Gelöscht |
| --- | --- | --- |
| `t_<uuid>` | `TestServices::start` in einem Test oder `kaeso-testkit fresh-services --temporary` | Wenn der Test endet, auch bei einer Panic; sonst beim Aufräumen |
| `dev_<uuid>` | `kaeso-testkit fresh-services`, für ein [aus dem Quellcode ausgeführtes Gateway](local-setup.md#das-gateway-aus-dem-quellcode-ausführen) | Nur durch `kaeso-testkit drop-services dev_<uuid>` |

Der erste Test eines Prozesses räumt `t_`-Datenbanken auf, die älter als sechs Stunden sind und nicht mehr genutzt werden. `KAESO_TEST_KEEP_DATABASES=1 cargo test …` behält die Datenbanken eines fehlgeschlagenen Tests und gibt ihre Namen aus. `KAESO_TEST_POSTGRES_URL`, `KAESO_TEST_CLICKHOUSE_URL` und `KAESO_TEST_VALKEY_URL` lenken die Tests auf andere Server.

### Snapshots und Referenzen

Manche Tests vergleichen mit geprüften `insta`-Snapshots. Ändert sich einer, den Diff prüfen und aktualisieren, was davon abhängt:

| Snapshot | Enthält | Ebenfalls aktualisieren |
| --- | --- | --- |
| `crates/kaeso-core/src/rights/snapshots/` | Jedes Recht und jede eingebaute Rolle | [Rechte](../concepts/rights.md) |
| `crates/kaeso-server/tests/snapshots/` | Jede Einstellung mit Standardwert und Beschreibung | [Konfiguration](../self-hosting/configuration.md) und `deploy/kaeso.example.toml` |
| `crates/kaeso-codecs/tests/snapshots/` | Die Ergebnisse der dialektübergreifenden Matrix | Die Mapping-Seite des Dialekts ([KPP-Interna](kpp/index.md#tests)) |
| `crates/kaeso-mail/src/snapshots/` | Text und HTML jeder E-Mail | |

Zwei weitere Tests wachen über Texte: `crates/kaeso-server/tests/error_copy.rs` prüft jede Meldung, die die Platform API senden kann, gegen die [Regeln für Fehlermeldungen](architecture/gateway.md#fehlermeldungen), und `crates/kaeso-server/tests/docs_paths.rs` prüft, dass jeder Repository-Pfad und jeder relative Link in den Markdown-Dateien im Wurzelverzeichnis und in `crates/`, `deploy/`, `e2e/`, `fuzz/` und `.github/` auf etwas Vorhandenes zeigt.

## End-to-End-Tests

Die Suites in `e2e/` führen die offiziellen Python-SDKs von OpenAI und Anthropic, festgelegt in `e2e/requirements.txt`, gegen die Encoder von KPP, das Gateway und eine frische Installation aus. Ihre virtuelle Umgebung einmal einrichten:

```bash
python3 -m venv e2e/.venv
e2e/.venv/bin/pip install -r e2e/requirements.txt
```

### SDK-Konformität

```bash
e2e/.venv/bin/pytest -q e2e/conformance
```

Diese Suite prüft, dass die SDKs alles akzeptieren, was die Encoder von KPP erzeugen. Sie baut und startet `kpp-conformance` (`crates/kpp-conformance`, ein Release-Build auf einem zufälligen Port); `KPP_CONFORMANCE_URL=http://127.0.0.1:8787` nutzt stattdessen einen Server, der mit `cargo run --release -p kpp-conformance` gestartet wurde.

```mermaid
sequenceDiagram
  accTitle: Eine Konformitätsanfrage
  accDescr: Das SDK sendet eine Anfrage, deren Modell einen Quelldialekt und eine Fixture nennt. Der Server decodiert sie mit dem Codec des Endpunkts, decodiert die Fixture mit dem Codec des Quelldialekts nach KPP, codiert das mit dem Codec des Endpunkts und gibt die Antwort, den Stream oder den Fehler zurück, den das SDK dann liest.
  participant SDK
  participant Server as kpp-conformance
  SDK->>Server: Anfrage, model "dialect:fixture"
  Note over Server: Mit dem Codec des Endpunkts decodieren
  Note over Server: Die Fixture mit dem Codec<br>des Quelldialekts nach KPP decodieren
  Note over Server: Mit dem Codec des Endpunkts codieren
  Server-->>SDK: Antwort, SSE-Stream oder Fehler
  Note over SDK: Liest sie
```

| Endpunkt | Codec | SDK-Aufrufe |
| --- | --- | --- |
| `POST /v1/messages` | `anthropic_messages` | `messages.create`, `messages.stream` |
| `POST /v1/messages/count_tokens` | `anthropic_messages` | `messages.count_tokens` |
| `POST /v1/chat/completions` | `openai_chat` | `chat.completions.create`, `chat.completions.stream` |
| `POST /v1/responses` | `openai_responses` | `responses.create`, `responses.stream` |
| `POST /v1/responses/input_tokens` | `openai_responses` | `responses.input_tokens.count` |
| `POST /v1/embeddings` | `openai_embeddings` | `embeddings.create` |
| `GET /v1/models`, `GET /v1/models/{id}` | `anthropic_messages` mit einem `anthropic-version`-Header, sonst `openai_chat` | `models.list`, `models.retrieve` |

Jede Antwort- und Stream-[Fixture](kpp/fixtures.md) jedes Generierungsdialekts wird über jeden Generierungs-Endpunkt ausgeliefert, im selben Dialekt und dialektübergreifend, gestreamt und nicht gestreamt. Die Tests vergleichen, was das SDK geparst hat, mit dem, was KPP dekodiert hat (`GET /_expected`), sodass eine Übersetzung, die Text, Tool-Argumente oder lesbares Reasoning verliert, fehlschlägt. Im Einzelnen:

- **Reasoning**, das das SDK bereitstellt (Zusammenfassungen und Text von Reasoning-Items in Responses, Text von `thinking`-Blöcken bei Anthropic), entspricht dem dekodierten Reasoning. In Responses-Streams ergeben die Ereignisse `response.reasoning_summary_text.delta` zusammen jedes `response.reasoning_summary_text.done` und die Zusammenfassungen der finalen Antwort.
- **Strukturierte Ausgaben** sind auch beim Lesen durch das SDK gültiges JSON.
- **Fehler:** Jede Fehler-Fixture jedes Dialekts, über die Endpunkte von Anthropic, Chat und Responses ausgeliefert, löst das `APIStatusError` des SDKs mit dem Status ihrer KPP-Fehlerart in diesem Format aus, behält `retry-after` bei Ratenlimits und nutzt den Fehler-Body des Formats (`e2e/conformance/test_errors.py`).
- **Modelllisten:** Die Modell-Endpunkte liefern einen festen Katalog von fünf Modellen, eines davon ausgemustert, mit `ModelList::page` geblättert. `e2e/conformance/test_models.py` folgt jeder Seite mit dem Auto-Paging des Anthropic-SDKs vorwärts (`limit=2`) und rückwärts (`before_id`), filtert nach `lifecycle`, erwartet `BadRequestError` für einen unbekannten Cursor oder `limit=0`, listet mit dem OpenAI-SDK, ruft Modelle ab und erwartet in beiden SDKs `NotFoundError` für unbekannte IDs.

`GET /_scenarios` listet jedes Szenario auf, das der Server anbietet.

### SDK-Suite

```bash
e2e/.venv/bin/pytest -q e2e/sdk
```

Diese Suite braucht die Entwicklungsdatenbanken. Sie baut das Gateway und `kaeso-testkit`, legt frische Datenbanken an, startet den nachgebildeten Anbieter und das Gateway und richtet über die Platform API den ersten Benutzer, eine Organisation und ein Projekt, Anbieter-Schlüssel, Katalogmodelle, einen Alias und API-Schlüssel ein. Die Tests decken Streaming, Keep-Alives bei einem stillen Anbieter, Tools, Bilder, PDFs, Reasoning, beide Richtungen zwischen Anbietern, Token-Zählung, Embeddings, Modelllisten, jede Fehlerklasse, Limits, SDK-Wiederholungen und Verbindungsabbrüche ab.

Der nachgebildete Anbieter (`crates/kaeso-testkit/src/mock_provider.rs`) wählt seine Antwort anhand von Markierungen in der Anfrage:

| Markierung | Wirkung |
| --- | --- |
| `[mock:fixture=NAME]` im Prompt | Antwortet mit dieser Fixture im Format des Anbieters |
| `[mock:gate=NAME]` im Prompt | Hält die Anfrage fest, bis der Test das Gate öffnet |
| Eine Modell-ID, die mit `unserved-` beginnt | Das „Modell nicht gefunden“ des Formats |
| `[mock:gate=NAME]` im Anbieter-Schlüssel | Hält `GET /v1/models` und `GET /v1/models/{id}` fest |
| `[mock:models=absent]` im Anbieter-Schlüssel | Beantwortet beide Modell-Endpunkte mit einem bloßen 404 |
| `[mock:models=list-only]` im Anbieter-Schlüssel | Beantwortet nur `GET /v1/models/{id}` mit einem bloßen 404 |
| `[mock:models=endless]` im Anbieter-Schlüssel | Hält die Anthropic-Liste auf ihrer ersten Seite |

`MockProvider::gate()` erzeugt ein Gate und gibt seine Markierung zurück; `wait_for(n)` kehrt zurück, sobald `n` Anfragen es erreicht haben, und `open()` lässt alle durch. So hält ein Test eine Anfrage ohne Wartezeit in der Schwebe.

### Installationstest

```bash
e2e/.venv/bin/pytest -q e2e/install
```

Diese Suite baut die Images von Gateway und nachgebildetem Anbieter aus `deploy/gateway.Dockerfile` und startet `deploy/compose.yaml` aus einem sauberen Zustand mit den Overrides in `e2e/install`. Sie schließt die Ersteinrichtung ab, ruft die Installation mit beiden SDKs auf und prüft, dass PostgreSQL und ClickHouse dem Gateway keine unnötigen Rechte geben und dass Valkey Verbindungen ohne sein Passwort ablehnt. Ein Platzhalter bedient die Adresse der Konsole; `KAESO_TEST_CONSOLE_IMAGE` startet stattdessen das echte Konsolen-Image, und `KAESO_TEST_GATEWAY_IMAGE` und `KAESO_TEST_MOCK_IMAGE` nutzen bereits gebaute Images.

## Fuzzing

`fuzz/` enthält ein [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html)-Target pro KPP-Decoder und eines für den SSE-Parser. Es ist ein eigener Cargo-Workspace außerhalb des Repository-Workspaces, wie cargo-fuzz es erwartet, sodass `libfuzzer-sys` nie die Bibliotheks-Crates erreicht.

| Target | Prüft |
| --- | --- |
| `decode_anthropic_messages`, `decode_openai_chat`, `decode_openai_responses`, `decode_openai_embeddings` | Jeder Decoder des Dialekts nimmt beliebige Bytes an, ohne in Panik zu geraten: Anfragen aller drei Arten, Antworten, Fehler, Modelllisten und Streams in Stücken, deren Größe die Eingabe bestimmt. Auch das Kodieren des Dekodierten in jeden Dialekt gerät nicht in Panik |
| `sse` | Der SSE-Parser gerät nie in Panik und liefert dieselben Elemente, wie auch immer die Eingabe aufgeteilt ist; die geparsten Elemente zurückgeschrieben und erneut geparst ergeben dieselben Elemente |

Die gemeinsame Logik steht in `fuzz/src/lib.rs`; jede Datei in `fuzz/fuzz_targets/` nennt ihren Dialekt. Jede Änderung prüft, dass die Targets bauen, formatiert sind und die Lints bestehen:

```bash
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo fmt --all --check --manifest-path fuzz/Cargo.toml
cargo clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings
```

Zum Ausführen der Targets braucht es eine Nightly-Toolchain und cargo-fuzz:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

Ein Target mit einem Korpus-Ordner und den Fixtures des Dialekts als Ausgangskorpus starten. Das Target `sse` nimmt jede Fixture:

```bash
mkdir -p fuzz/corpus/decode_openai_chat
cargo +nightly fuzz run decode_openai_chat fuzz/corpus/decode_openai_chat crates/kaeso-codecs/fixtures/openai_chat -- -max_total_time=600 -rss_limit_mb=2048 -max_len=65536
```

```bash
mkdir -p fuzz/corpus/sse
cargo +nightly fuzz run sse fuzz/corpus/sse crates/kaeso-codecs/fixtures -- -max_total_time=600
```

Ein Absturz landet in `fuzz/artifacts/<target>/`. Ihn mit `cargo +nightly fuzz run <target> <file>` reproduzieren, daraus einen Regressionstest im Codec machen und dann den Codec korrigieren. [Performance](performance.md#fuzzing) hält den letzten vollständigen Lauf fest.

## Continuous Integration

Der CI-Workflow (`.github/workflows/ci.yml`) läuft bei jedem Pull Request, bei jedem Push auf `dev` und `prod` und innerhalb des Release-Workflows. Die eigenen Pushes des Release-Workflows, sein Release-Commit und das nach `dev` gebrachte Release, starten ihn nicht erneut, denn das Release hat jeden Job bereits bestanden.

Bei einem Pull Request entscheidet der erste Job, welche Bereiche sich geändert haben, und die Jobs unveränderter Bereiche werden übersprungen; in allen anderen Fällen läuft jeder Job. Die einzige Pflichtprüfung ist **CI**: Sie besteht, wenn **Changes** und **No comments** bestanden haben und jeder andere Job bestanden hat oder übersprungen wurde. Ein Pull Request nach `dev` lässt sich erst mergen, wenn sie grün ist ([wie Änderungen nach dev gelangen](contributing.md#wie-änderungen-nach-dev-gelangen)).

| Job | Führt aus |
| --- | --- |
| Changes | Entscheidet, welche Bereiche ein Pull Request ändert: Rust, TypeScript, Deployment, die eigenen Dateien des Repositorys, die Workflows |
| No comments | Keine Kommentare in Quelldateien, Konfiguration oder Markdown-Codeblöcken |
| Workflows | `actionlint` |
| Rust format | `cargo fmt --all --check`, für den Workspace und `fuzz/` |
| Rust lint | `cargo clippy` mit jeder Warnung als Fehler, für den Workspace und `fuzz/` |
| Test KPP | `cargo coverage-kpp` und `cargo test --doc -p kaeso-kpp -p kaeso-codecs` |
| Minimum Rust version | `cargo check` und `cargo test` von `kaeso-kpp` und von `kaeso-codecs`, jeweils mit der Rust-Version, die ihre `rust-version` nennt |
| Test gateway | `cargo coverage-gateway`, mit dem Entwicklungs-Stack |
| Repository tooling | `node scripts/version.mjs check`, `cargo test -p xtask -p kpp-conformance` und `cargo test -p kaeso-server --test docs_paths` |
| Query data | Die PostgreSQL-Migrationen, dann `cargo sqlx prepare --workspace --check` |
| OpenAPI document | `cargo run -q -p kaeso -- openapi \| diff - openapi.json` |
| Rust dependencies | `cargo deny check`, mit der Richtlinie des Workspaces und der von KPP |
| Fuzz targets | `cargo check --manifest-path fuzz/Cargo.toml --bins` |
| Benchmarks build | `cargo bench -p kaeso-codecs --bench codecs --no-run` |
| SDK conformance | `pytest -q e2e/conformance` |
| SDK end-to-end | `pytest -q e2e/sdk`, mit dem Entwicklungs-Stack |
| Compose config | `docker compose config` für `deploy/compose.yaml` und `deploy/compose.dev.yaml` |
| Images | Baut die Images von Gateway, nachgebildetem Anbieter und Konsole, beschriftet mit den Versionen aus `Cargo.toml` und `apps/console/package.json`, und prüft, dass das Konsolen-Image `/healthz` beantwortet, die Konsole mit einer Content Security Policy ausliefert und für eine fehlende Datei unter `/assets/` 404 antwortet |
| Install test | `pytest -q e2e/install` mit diesen Images, die echte Konsole eingeschlossen |
| TypeScript lint | `pnpm format:check`, `pnpm lint`, `pnpm i18n` und `pnpm knip` |
| TypeScript typecheck | `pnpm typecheck` |
| TypeScript test | `pnpm test`, mit der Coverage jedes Pakets in der Zusammenfassung des Jobs |
| TypeScript build | `pnpm build`, `pnpm licenses:check` und `pnpm release:check` |
| End-to-end console | `pnpm --filter kaeso-console e2e` gegen das Gateway-Image |
| End-to-end docs | `pnpm --filter kaeso-docs e2e` |
| End-to-end showcase | `pnpm --filter kaeso-showcase e2e` gegen den Showcase, den TypeScript build gebaut hat |
| TypeScript dependencies | `pnpm audit --prod --audit-level high` |
| CI | Die Pflichtprüfung |
