---
title: Tests
description: Jede Prüfung, die eine Änderung bestehen muss, was jede Test-Suite abdeckt und wie Sie sie ausführen.
order: 4
---

Jede Änderung bringt Tests mit, die prüfen, was eine Person oder eine App sieht. Befehle laufen im Wurzelverzeichnis des Repositorys. Suites, die Datenbanken brauchen, benötigen den [Entwicklungs-Stack](stack.md).

## Bevor eine Änderung fertig ist

Jede Änderung besteht diese Prüfungen:

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

Führen Sie dann diese für die Teile aus, die Sie geändert haben:

| Geändert | Außerdem ausführen |
| --- | --- |
| KPP | Die [SDK-Konformität](#sdk-konformität), `cargo test --doc -p kaeso-kpp -p kaeso-codecs` und die [Prüfungen der Fuzz-Targets](#fuzzing) |
| Das Gateway | Die [SDK-Suite](#sdk-suite) |
| `deploy/`, ein Dockerfile, die Paketierung | Den [Installationstest](#installationstest) und `docker build -f deploy/console.Dockerfile .` |
| `.github/workflows` | `actionlint` |
| Eine Oberfläche | Sehen Sie sich jeden geänderten Bildschirm in Hell und Dunkel, in Desktop- und Telefonbreite, auf Englisch, Deutsch und Chinesisch an |

Die [Continuous Integration](#continuous-integration) führt dieselben Prüfungen für jeden Pull Request aus.

## TypeScript

| Befehl | Prüft |
| --- | --- |
| `pnpm lint` | ESLint oder oxlint in jedem Paket, mit Kaesos eigenen Regeln: Klassennamen nur für Layout, keine Farben außerhalb der Tokens, kein direkt in die Konsole geschriebener Text. Für die Dokumentation jede Seite mit `check:content` |
| `pnpm format:check` | Prettier |
| `pnpm typecheck` | TypeScript, nachdem die Bibliotheken in `packages/` gebaut sind |
| `pnpm test` | Vitest mit Coverage-Schwellen in beiden Bibliotheken, der Konsole und der Dokumentation. Danach die Tests von `scripts/`, darunter die [Versionsprüfung](releasing.md#die-version-im-repository) |
| `pnpm i18n` | Jeder Text der Konsole hat Deutsch und Chinesisch mit denselben Platzhaltern, und nichts ist übrig ([Übersetzungen](translations.md#prüfen)) |
| `pnpm knip` | Ungenutzte Dateien, Exporte und Abhängigkeiten |
| `pnpm build` | Jeden Produktions-Build und seine Bundle-Budgets. Die Builds beider Websites führen danach `scripts/check-site.mjs` aus, das die Sitemap und den `<head>` jeder Seite prüft |
| `pnpm licenses:check` | Nach einem Build: die Lizenz jedes Pakets, dessen Code ausgeliefert wird ([Lizenzrichtlinie](contributing.md#abhängigkeit-hinzufügen)) |
| `pnpm release:check` | Nach einem Build: beide Bibliotheken so gepackt, wie npm es tut, mit jeder Datei, die sie ausliefern müssen, in einer Version |
| `pnpm e2e` | Playwright in Konsole, Dokumentation und Showcase, nacheinander. Die Suite des Showcases testet dessen Build, führen Sie also zuerst `pnpm build` aus |

`pnpm --filter <package> <script>` führt das Skript eines Pakets aus. Die Pakete heißen `kaeso-console`, `kaeso-docs`, `kaeso-showcase`, `@logicplanes/kaeso-ui` und `@logicplanes/kaeso-tokens`. Nachdem Sie eine Bibliothek geändert haben, führen Sie vor einem gefilterten Lauf `pnpm build:packages` aus. Die Coverage-Schwellen jedes Pakets stehen in seiner `vitest.config.ts` und steigen nur.

Playwright braucht Chromium, das Sie einmalig installieren:

```bash
pnpm --filter kaeso-console exec playwright install chromium
```

### Die Konsole

Unit-Tests laufen in happy-dom gegen die Platform API, die pro Test gestubbt wird (`apps/console/src/test/network.ts`, mit Fixtures in `src/test/fixtures/`). `renderApp(path)` bindet die ganze Konsole unter einer Adresse ein, `renderUi` eine einzelne Komponente. Tests prüfen, was eine Person sieht und welche Anfragen gesendet werden, und bestehen in jeder Reihenfolge. Um die Reihenfolge zu prüfen, führen Sie `pnpm --filter kaeso-console exec vitest run --sequence.shuffle` aus. Testdateien unter `src/routes` beginnen mit `-`, damit der Routengenerator sie überspringt.

Die End-to-End-Suite berührt den Entwicklungs-Stack nie. Sie startet einen eigenen, das Compose-Projekt `kaeso-console-e2e`, mit dem Gateway auf `127.0.0.1:28080` und der Konsole unter http://localhost:3100. Stack und Daten werden am Ende des Laufs entfernt. Um den Stack zwischen Läufen weiterlaufen zu lassen, starten Sie ihn selbst und teilen es der Suite mit:

```bash
node apps/console/scripts/stack.mjs up --e2e
KAESO_E2E_STACK=running pnpm --filter kaeso-console e2e
```

Die Specs bauen aufeinander auf, beginnend mit `01-public`, das die Installation einrichtet. CI führt sie in fünf Teilen aus, und eine neue Spec läuft in Teil 5, sofern `PART_1`, `PART_2`, `PART_3` oder `PART_4` in `.github/workflows/ci.yml` sie nicht nennt. Ein Test schlägt bei jedem Fehler fehl, den der Browser meldet, auch bei einem `console.error`. Specs importieren deshalb `test` und `expect` aus `apps/console/e2e/support.ts`.

### Die Dokumentations-Website

Unit-Tests und End-to-End-Suite nutzen die Fixture-Seiten in `apps/docs/e2e/fixtures/content`, nie die echten Seiten, sodass eine Änderung an der Dokumentation nie einen Test bricht. `pnpm --filter kaeso-docs e2e` führt Playwright samt axe gegen einen Produktions-Build der Fixtures aus, in jeder Sprache und in Telefonbreite. `pnpm --filter kaeso-docs check:content` und der Build prüfen die echten Seiten. Die Tests des Diagramm-Renderers zeichnen in Chromium, daher braucht `pnpm test` das Chromium, das Playwright installiert.

### Die UI-Bibliothek und der Showcase

Komponententests laufen in happy-dom mit Testing Library. Der Showcase hat keine Unit-Tests. Seine End-to-End-Suite führt Playwright gegen seinen Produktions-Build aus, führen Sie also zuerst `pnpm build` aus. Sie prüft die Barrierefreiheit mit axe in beiden Themes sowie Tastatur und Fokusringe. Außerdem prüft sie Schriftskala, Bewegung und Geometrie der Tokens, jede Komponentenseite in Telefonbreite, die deutschen und chinesischen Seiten sowie Sitemap und `<head>` der Seiten.

## Rust

Installieren Sie die Cargo-Werkzeuge einmalig:

```bash
cargo install cargo-deny cargo-llvm-cov cargo-nextest sqlx-cli --locked
```

| Befehl | Prüft |
| --- | --- |
| `cargo fmt --all --check` | Formatierung (`rustfmt.toml`) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Lints, darunter kein `unwrap`, `expect`, `panic!` oder panikauslösendes Indexieren außerhalb von Tests (`Cargo.toml`, `clippy.toml`) |
| `cargo test --workspace --all-features` | Jeden Test |
| `cargo coverage-kpp` | Die Tests von `kaeso-kpp` und `kaeso-codecs` unter `cargo llvm-cov`, mit ihrer Coverage-Schwelle |
| `cargo test --doc -p kaeso-kpp -p kaeso-codecs` | Die Beispiele in den READMEs beider Crates, ihrer Dokumentation auf docs.rs, die `cargo llvm-cov` nicht ausführt |
| `cargo coverage-gateway` | Die Tests aller anderen Crates außer `kpp-conformance` und `xtask` unter `cargo llvm-cov nextest`, jeder Test in einem eigenen Prozess und alle gleichzeitig, mit ihrer Coverage-Schwelle. Ein Test, der nach 5 Minuten noch läuft, wird beendet und schlägt fehl |
| `cargo test -p xtask -p kpp-conformance` | Die Aufgaben des Repositorys und den Konformitätsserver, die die Coverage-Befehle auslassen |
| `cargo deny check` | Lizenzen, Sicherheitshinweise, Verbote und Quellen jeder Abhängigkeit (`deny.toml`) |
| `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` | Die strengere Richtlinie von KPP: keine Runtime- oder HTTP-Crates in der Bibliothek |
| `cargo sqlx prepare --workspace --check` | Die Offline-Abfragedaten in `.sqlx/` passen zu den Abfragen |
| `cargo run -q -p kaeso -- openapi \| diff - openapi.json` | `openapi.json` ist aktuell |

Die Coverage-Befehle sind Aliasse in `.cargo/config.toml`, wo auch ihre Schwellen und die ausgelassenen Dateien stehen. Tests, die die Datenbanken brauchen, schlagen fehl, statt übersprungen zu werden, wenn die Datenbanken fehlen.

`cargo sqlx prepare --check` braucht `DATABASE_URL=postgres://kaeso:kaeso@127.0.0.1:55432/kaeso` mit angewandten Migrationen. Das Gateway des Stacks wendet sie beim Start an. Ohne den Stack führen Sie `cargo sqlx migrate run --source crates/kaeso-store/migrations/postgres` aus.

`kaeso-kpp` und `kaeso-codecs` nennen das älteste unterstützte Rust als `rust-version` in ihrer `Cargo.toml`, und CI testet jedes Crate damit. Um das selbst zu tun, installieren Sie diese Version mit `rustup toolchain install <version>` und führen `cargo +<version> test --locked -p <crate>` aus.

### Testdatenbanken

Jeder Test legt eigene Datenbanken auf den Entwicklungsinstanzen von PostgreSQL, ClickHouse und Valkey an, sodass Tests ohne gemeinsamen Zustand parallel laufen. `kaeso-testkit` benennt und entfernt sie:

| Name | Angelegt von | Gelöscht |
| --- | --- | --- |
| `t_<uuid>` | `TestServices::start` in einem Test oder `kaeso-testkit fresh-services --temporary` | Wenn der Test endet, auch bei einer Panic, oder beim Aufräumen |
| `dev_<uuid>` | `kaeso-testkit fresh-services`, für ein [aus dem Quellcode gestartetes Gateway](stack.md#gateway-aus-dem-quellcode-starten) | Nur durch `kaeso-testkit drop-services dev_<uuid>` |

Der erste Test eines Prozesses räumt `t_`-Datenbanken auf, die älter als sechs Stunden sind und nicht mehr genutzt werden. Um die Datenbanken eines fehlgeschlagenen Tests zu behalten, führen Sie ihn mit `KAESO_TEST_KEEP_DATABASES=1` aus. Er gibt dann ihre Namen aus.

### Snapshots und Referenzen

Manche Tests vergleichen mit geprüften `insta`-Snapshots. Ändert sich einer, prüfen Sie den Diff und aktualisieren, was davon abhängt:

| Snapshot | Enthält | Ebenfalls aktualisieren |
| --- | --- | --- |
| `crates/kaeso-core/src/rights/snapshots/` | Jedes Recht und jede eingebaute Rolle | [Rechte](../concepts/rights.md) |
| `crates/kaeso-server/tests/snapshots/` | Jede Einstellung mit Standardwert und Beschreibung | [Konfiguration](../self-hosting/configuration.md) und `deploy/kaeso.example.toml` |
| `crates/kaeso-codecs/tests/snapshots/` | Die Ergebnisse der dialektübergreifenden Matrix | Die Mapping-Seite des Dialekts ([KPP-Interna](kpp/index.md#tests)) |
| `crates/kaeso-mail/src/snapshots/` | Text und HTML jeder E-Mail | |

Zwei weitere Tests wachen über Texte. `crates/kaeso-server/tests/error_copy.rs` prüft jede Meldung, die die Platform API senden kann, gegen die [Regeln für Fehlermeldungen](architecture/gateway.md#fehlermeldungen). `crates/kaeso-server/tests/docs_paths.rs` prüft, dass jeder Repository-Pfad und jeder relative Link in den Markdown-Dateien im Wurzelverzeichnis und in `crates/`, `deploy/`, `e2e/`, `fuzz/` und `.github/` auf etwas Vorhandenes zeigt.

## End-to-End-Tests

Die Suites in `e2e/` führen die offiziellen Python-SDKs von OpenAI und Anthropic, festgelegt in `e2e/requirements.txt`, gegen die Encoder von KPP, das Gateway und eine frische Installation aus. Richten Sie ihre virtuelle Umgebung einmalig ein:

```bash
python3 -m venv e2e/.venv
e2e/.venv/bin/pip install -r e2e/requirements.txt
```

### SDK-Konformität

```bash
e2e/.venv/bin/pytest -q e2e/conformance
```

Diese Suite prüft, dass die SDKs alles annehmen, was die Encoder von KPP erzeugen. Sie baut und startet `kpp-conformance`, einen Server in `crates/kpp-conformance`, auf einem zufälligen Port. Um einen Server zu nutzen, den Sie mit `cargo run --release -p kpp-conformance` gestartet haben, setzen Sie `KPP_CONFORMANCE_URL=http://127.0.0.1:8787`.

```mermaid
sequenceDiagram
  accTitle: Eine Konformitätsanfrage
  accDescr: Das SDK sendet eine Anfrage, deren Modell einen Quelldialekt und eine Fixture nennt. Der Server dekodiert sie mit dem Codec des Endpunkts, dekodiert die Fixture mit dem Codec des Quelldialekts in KPP, kodiert das mit dem Codec des Endpunkts und gibt Antwort, Stream oder Fehler zurück, die das SDK parst.
  participant SDK
  participant Server as kpp-conformance
  SDK->>Server: Anfrage, Modell „dialect:fixture“
  Note over Server: Mit dem Codec des Endpunkts dekodieren
  Note over Server: Die Fixture mit dem Codec des<br>Quelldialekts in KPP dekodieren
  Note over Server: Mit dem Codec des Endpunkts kodieren
  Server-->>SDK: Antwort, SSE-Stream oder Fehler
  Note over SDK: Parst sie
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

Jede Antwort- und Stream-[Fixture](kpp/fixtures.md) jedes Generierungsdialekts läuft durch jeden Generierungsendpunkt, gestreamt und nicht gestreamt. Die Tests vergleichen, was das SDK geparst hat, mit dem, was KPP dekodiert hat (`GET /_expected`). Eine Übersetzung, die Text, Tool-Argumente oder lesbares Reasoning verliert, schlägt deshalb fehl. Die Tests prüfen außerdem, dass strukturierte Ausgabe gültiges JSON bleibt und dass jede Fehler-Fixture den `APIStatusError` des SDKs mit dem richtigen Status auslöst (`e2e/conformance/test_errors.py`). Modelllisten müssen so blättern und filtern, wie die SDKs es erwarten (`e2e/conformance/test_models.py`). `GET /_scenarios` listet jedes Szenario auf, das der Server anbietet.

### SDK-Suite

```bash
e2e/.venv/bin/pytest -q e2e/sdk
```

Diese Suite führt die SDKs gegen das Gateway aus, auf den Datenbanken der Entwicklung. Sie baut das Gateway und `kaeso-testkit`, startet den Mock-Anbieter und das Gateway und richtet Benutzer, Projekte, Anbieter-Schlüssel, Modelle und API-Schlüssel über die Platform API ein. Die Tests decken Streaming, Keepalives, Tools, Bilder, PDFs, Reasoning, beide Richtungen zwischen Anbietern, das Zählen von Token, Embeddings, Modelllisten, jede Fehlerklasse, Limits, Wiederholungen der SDKs und Verbindungsabbrüche ab.

Der Mock-Anbieter (`crates/kaeso-testkit/src/mock_provider.rs`) wählt seine Antwort anhand von Markierungen in der Anfrage:

| Markierung | Wirkung |
| --- | --- |
| `[mock:fixture=NAME]` im Prompt | Antwortet mit dieser Fixture im Format des Anbieters; `providers/<id>/<name>` wählt die Aufzeichnung eines Anbieters |
| `[mock:gate=NAME]` im Prompt | Hält die Anfrage fest, bis der Test das Gate öffnet |
| `[mock:steps=NAME]` im Prompt | Sendet das erste Event eines Streams, dann ein weiteres bei jedem Schritt, den der Test das Gate weiterschaltet |
| Eine Modell-ID, die mit `unserved-` beginnt | Den Fehler „Modell nicht gefunden“ im Format des Anbieters |
| `[mock:gate=NAME]` im Anbieter-Schlüssel | Hält `GET /v1/models` und `GET /v1/models/{id}` fest |
| `[mock:models=absent]` im Anbieter-Schlüssel | Beantwortet beide Modell-Endpunkte mit einem bloßen 404 |
| `[mock:models=list-only]` im Anbieter-Schlüssel | Beantwortet nur `GET /v1/models/{id}` mit einem bloßen 404 |
| `[mock:models=endless]` im Anbieter-Schlüssel | Hält die Anthropic-Liste auf ihrer ersten Seite |

`MockProvider::gate()` erzeugt ein Gate und gibt seine Markierung zurück. `wait_for(n)` kehrt zurück, sobald `n` Anfragen es erreicht haben, und `open()` lässt sie alle durch. So hält ein Test eine Anfrage ohne Sleep in der Schwebe.

### Installationstest

```bash
e2e/.venv/bin/pytest -q e2e/install
```

Diese Suite baut die Images des Gateways und des Mock-Anbieters und startet `deploy/compose.yaml` aus einem sauberen Zustand, mit den Overrides in `e2e/install`. Sie schließt die Einrichtung beim ersten Start ab und ruft die Installation mit beiden SDKs auf. Sie prüft außerdem, dass PostgreSQL und ClickHouse dem Gateway keine Rechte geben, die es nicht braucht, und dass Valkey Verbindungen ohne sein Passwort ablehnt. Ein Platzhalter bedient die Adresse der Konsole. `KAESO_TEST_CONSOLE_IMAGE` startet stattdessen das echte Image der Konsole, und `KAESO_TEST_GATEWAY_IMAGE` und `KAESO_TEST_MOCK_IMAGE` nutzen bereits gebaute Images.

## Fuzzing

`fuzz/` enthält ein [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html)-Target pro KPP-Decoder, `decode_<dialect>`, und eines für den SSE-Parser, `sse`. Die Decoder-Targets prüfen, dass jeder Decoder beliebige Bytes ohne Panic annimmt und dass auch das Kodieren des Ergebnisses in jeden Dialekt keine Panic auslöst. Das Target `sse` prüft, dass der Parser dieselben Einträge liefert, egal wie die Eingabe geteilt ist. `fuzz/` ist ein eigener Cargo-Workspace, sodass `libfuzzer-sys` nie die Bibliotheks-Crates erreicht.

Jede Änderung prüft, dass die Targets bauen und formatiert und gelintet sind:

```bash
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo fmt --all --check --manifest-path fuzz/Cargo.toml
cargo clippy --manifest-path fuzz/Cargo.toml --all-targets -- -D warnings
```

Um die Targets auszuführen, brauchen Sie eine Nightly-Toolchain und cargo-fuzz:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

Führen Sie ein Target mit einem Korpus-Ordner und den Fixtures des Dialekts als Startkorpus aus. Das Target `sse` nimmt jede Fixture:

```bash
mkdir -p fuzz/corpus/decode_openai_chat
cargo +nightly fuzz run decode_openai_chat fuzz/corpus/decode_openai_chat crates/kaeso-codecs/fixtures/openai_chat -- -max_total_time=600 -rss_limit_mb=2048 -max_len=65536
```

Der Workflow **Fuzz** lässt jedes Target jede Nacht 10 Minuten laufen und 60 Sekunden bei jedem Pull Request, der KPP oder `fuzz/` ändert. Er schlägt bei einem Absturz fehl und behält die Eingabe als Artefakt `fuzz-crash-<target>`. Ein Absturz landet in `fuzz/artifacts/<target>/`. Reproduzieren Sie ihn mit `cargo +nightly fuzz run <target> <file>`, machen Sie daraus einen Regressionstest im Codec und korrigieren Sie dann den Codec.

## Continuous Integration

Der CI-Workflow, `.github/workflows/ci.yml`, läuft für jeden Pull Request, bei jedem Push auf `dev` und `prod` und innerhalb des Release-Workflows. Bei einem Pull Request entscheidet sein erster Job, welche Bereiche sich geändert haben, und die Jobs unveränderter Bereiche werden übersprungen. Überall sonst läuft jeder Job.

Die einzige erforderliche Prüfung ist **CI**. Sie besteht, wenn **Changes** und **Comments** bestanden haben und jeder andere Job bestanden hat oder übersprungen wurde. Ein Pull Request nach `dev` lässt sich erst mergen, wenn sie grün ist ([Pull Request mergen lassen](contributing.md#pull-request-mergen-lassen)).

| Job | Führt aus |
| --- | --- |
| Changes | Entscheidet, welche Bereiche ein Pull Request ändert |
| Comments | Keine `TODO`-, `FIXME`-, `XXX`- oder `HACK`-Notizen in Kommentaren und keine Kommentare in Codeblöcken in Markdown |
| Workflows | `actionlint` |
| Rust format, Rust lint | `cargo fmt` und `cargo clippy`, für den Workspace und `fuzz/` |
| Test KPP | `cargo coverage-kpp` und die Doc-Tests von `kaeso-kpp` und `kaeso-codecs` |
| Minimum Rust version | `cargo check` und `cargo test` jedes KPP-Crates mit seiner `rust-version` |
| Test gateway | `cargo coverage-gateway`, mit dem Entwicklungs-Stack |
| Repository tooling | `node scripts/version.mjs check`, `cargo test -p xtask -p kpp-conformance` und `cargo test -p kaeso-server --test docs_paths` |
| Query data | Die PostgreSQL-Migrationen, dann `cargo sqlx prepare --workspace --check` |
| OpenAPI document | `openapi.json` ist aktuell |
| Rust dependencies | `cargo deny check`, mit der Richtlinie des Workspaces und der von KPP |
| Fuzz targets | Die Fuzz-Targets bauen |
| Published crates | `cargo publish --dry-run` und `cargo doc` mit Warnungen als Fehler für `kaeso-kpp` und `kaeso-codecs`, und `cargo semver-checks`: gegen die zuletzt veröffentlichte Version für ein Minor-Release und bei einem Pull Request gegen seine Basis für ein Patch-Release, außer er trägt das Label `breaking` |
| Fuzz runs | Der Workflow **Fuzz** mit 60 Sekunden je Target, wenn sich KPP oder `fuzz/` geändert hat |
| SDK conformance, SDK end-to-end | `pytest -q e2e/conformance` und `pytest -q e2e/sdk` |
| Compose config | `docker compose config` für beide Compose-Dateien |
| Images (gateway, mock provider, updater, console) | Bauen die vier Images; der Job der Konsole prüft außerdem, dass ihr Image die Konsole mit einer Content Security Policy ausliefert |
| Install test | `pytest -q e2e/install` mit diesen Images |
| TypeScript lint | `pnpm format:check`, `pnpm lint`, `pnpm i18n` und `pnpm knip` |
| TypeScript typecheck | `pnpm typecheck` |
| TypeScript libraries | `pnpm build:packages`, einmal für jeden Job, der die Bibliotheken braucht |
| TypeScript test | `pnpm test`, ein Job pro Workspace; die Konsole in sechs Shards, deren Coverage ein weiterer Job zusammenführt und prüft |
| TypeScript build | `pnpm build`, `pnpm licenses:check` und `pnpm release:check` |
| End-to-end console, docs und showcase | Das `e2e` jedes Pakets; die Konsole in fünf Teilen, der Showcase in Shards, seine Barrierefreiheitsprüfungen in eigenen Shards |
| TypeScript dependencies | `pnpm audit --prod --audit-level high` |

Schlägt ein End-to-End-Shard fehl, führt der Job **End-to-end console report** oder **End-to-end showcase report** die Ergebnisse der Shards zu einem HTML-Bericht zusammen, dem Artefakt `playwright-console` oder `playwright-showcase`.

Zwei Workflows laufen nach Zeitplan. **Fuzz** läuft jede Nacht, wie unter [Fuzzing](#fuzzing) beschrieben. **Audits** läuft jeden Montag: `cargo deny check` für den Workspace und KPP und `pnpm audit --audit-level high` für alle Abhängigkeiten, auch die für die Entwicklung. So fällt eine neue Sicherheitsmeldung auf, auch wenn sich nichts ändert.
