---
title: Lokale Einrichtung
description: Schritt für Schritt von einem frischen Klon zu einer laufenden Installation mit Beispieldaten und bestandenen Tests.
order: 1
---

Diese Seite richtet alles auf dem eigenen Rechner ein: das aus dem Quellcode gebaute Gateway mit seinen Datenbanken, die Konsole, Beispieldaten und die Tests. Jeder Befehl läuft vom Wurzelverzeichnis des Repositorys aus.

## Was benötigt wird

| Werkzeug | Version | Genutzt für |
| --- | --- | --- |
| [Git](https://git-scm.com) | jede aktuelle | Den Code |
| [Node.js](https://nodejs.org) | 24, wie in `.node-version` | Die Konsole, die UI-Bibliothek, die Dokumentation und die Skripte |
| pnpm | 11, in `package.json` festgelegt und von Corepack installiert | Den TypeScript-Workspace |
| [Rust](https://rustup.rs) über rustup | 1.99, in `rust-toolchain.toml` festgelegt und bei der ersten Nutzung installiert | Das Gateway und KPP |
| [Docker](https://docs.docker.com/get-docker/) mit Compose | Docker Compose 2.24 oder neuer | Den Entwicklungs-Stack |
| [Python](https://www.python.org) | 3.13 | Nur für die [End-to-End-Tests](testing.md#end-to-end-tests) |

Corepack ist bei Node.js dabei. Einmal einschalten, dann stellt es genau die pnpm-Version bereit, die das Repository festlegt:

```bash
corepack enable
```

rustup liest `rust-toolchain.toml`, sobald `cargo` zum ersten Mal im Repository läuft, und installiert Rust 1.99 mit rustfmt, clippy und llvm-tools. `rustup show` erledigt das vorab.

## 1. Code holen

```bash
git clone https://github.com/logicplanes/kaeso
cd kaeso
```

## 2. Abhängigkeiten installieren

```bash
pnpm install
```

Das installiert jedes Paket des TypeScript-Workspace: die Konsole, die Dokumentation, den Showcase und die Bibliotheken in `packages/`. Die Rust-Abhängigkeiten holt Cargo beim ersten Build von selbst.

## 3. Entwicklungs-Stack starten

```bash
pnpm stack:up
```

Das baut das Image des Gateways aus dem eigenen Checkout (`kaeso-dev-gateway`, aus `deploy/gateway.Dockerfile`), startet es in Docker mit PostgreSQL, ClickHouse, Valkey, Mailpit, einem Mock-Modellanbieter und einem Mock-Identitätsanbieter und wartet dann, bis jeder Dienst healthy ist. Der erste Build kompiliert das Gateway und dauert einige Minuten; spätere nutzen den Cache von Docker.

| Dienst | Was er ist | Auf dem eigenen Rechner |
| --- | --- | --- |
| `gateway` | Das aus dem eigenen Checkout gebaute Gateway | `127.0.0.1:18080` |
| `postgres`, `clickhouse`, `valkey` | Die Datenbanken des Gateways, auch von den Rust-Tests genutzt | `127.0.0.1:55432`, `127.0.0.1:58123`, `127.0.0.1:56379` |
| `mail` | Mailpit, das Anmeldelinks, Einladungen und Passwort-Zurücksetzungen abfängt | `http://127.0.0.1:58025`, SMTP auf `127.0.0.1:51025` |
| `provider` | Eine Mock-API für OpenAI und Anthropic, die jede Anfrage beantwortet, ohne einen echten Anbieter aufzurufen | Nur innerhalb des Stacks |
| `idp` | Ein Mock-OpenID-Connect-Anbieter für Single Sign-on | `http://idp.localhost:18091` |
| `master-key` | Schreibt den Hauptschlüssel des Gateways in ein Volume, bevor es startet | |

Der Stack ist das Compose-Projekt `kaeso-dev`, definiert in `deploy/compose.dev.yaml`. Ein zweiter Klon kann daneben einen eigenen Stack betreiben, wie unten unter Einen zweiten Stack betreiben beschrieben. Jede Modellanfrage geht an den Mock-Anbieter, daher ist kein echter Anbieter-Schlüssel nötig.

## 4. Mit Beispieldaten füllen

```bash
pnpm seed
```

Das richtet die Installation über die Platform API ein, so wie es eine Person tun würde, und füllt sie: die Organisation Northwind Labs mit fünf Projekten, sechs Mitgliedern, Schlüsseln, Modellen, Aliassen und Limits, einige hundert Anfragen, die über das Gateway an den Mock-Anbieter gesendet werden, und 70 Tage ältere Anfragehistorie.

Um die Installation stattdessen von Hand einzurichten, diesen Schritt überspringen: `pnpm stack:up` gibt einen Einrichtungslink aus (`http://localhost:3000/setup#token=…`), solange die Installation keine Benutzer hat.

## 5. Konsole öffnen

```bash
pnpm dev
```

Das baut die Bibliotheken in `packages/`, startet den Stack (und baut das Gateway neu, wenn sich sein Quellcode geändert hat) und stellt die Konsole unter http://localhost:3000 bereit. Sie lädt beim Speichern neu.

Als `owner@example.com` anmelden, Instanz-Admin und Inhaber der Organisation, oder als `lin@example.com`, ein Entwickler. Beide haben das Passwort `correct horse battery staple`. E-Mails, die das Gateway sendet, landen in Mailpit unter http://127.0.0.1:58025.

```text
browser ─▶ http://localhost:3000 (Vite) ─▶ /platform, /v1 sent on to the gateway on 127.0.0.1:18080
                                         └▶ everything else: the console
gateway ─▶ provider (mock OpenAI and Anthropic), mail (Mailpit), idp (mock single sign-on)
```

Die öffentliche URL des Gateways ist die Adresse des Entwicklungsservers, sodass das Session-Cookie ein First-Party-Cookie ist und der `Origin`, den der Browser sendet, der ist, den das Gateway erwartet. Im Produktivbetrieb laufen Konsole und Gateway auf dieselbe Weise unter einer Domain, hinter einem Reverse Proxy.

## 6. Tests ausführen

Die TypeScript-Tests:

```bash
pnpm test
```

Die Rust-Tests, die die Datenbanken des Stacks nutzen:

```bash
cargo test --workspace --all-features
```

Jeder Rust-Test legt eigene Datenbanken an, sodass Tests parallel laufen und die Beispieldaten unberührt lassen. [Tests](testing.md) listet jede Suite, die End-to-End-Tests und die Prüfungen, die eine Änderung bestehen muss.

## Stack stoppen

```bash
pnpm stack:down
```

Das entfernt den Stack mit seinen Daten, einschließlich der Datenbanken der Rust-Tests; das nächste `pnpm stack:up` startet leer.

## Ports und Images ändern

Die Ports des Stacks kommen aus `apps/console/scripts/stack.mjs`. Diese Variablen ändern sie: `KAESO_GATEWAY_PORT`, `KAESO_POSTGRES_PORT`, `KAESO_CLICKHOUSE_PORT`, `KAESO_VALKEY_PORT`, `KAESO_MAIL_PORT`, `KAESO_MAIL_SMTP_PORT`, `KAESO_IDP_PORT` und `KAESO_PUBLIC_URL`. Liegen die Datenbanken woanders, finden die Rust-Tests sie über `KAESO_TEST_POSTGRES_URL`, `KAESO_TEST_CLICKHOUSE_URL` und `KAESO_TEST_VALKEY_URL`.

Um den Stack mit einem anderswo gebauten Gateway-Image zu betreiben, etwa einem Release, dieses angeben; der Stack nutzt es dann, statt selbst zu bauen:

```bash
KAESO_GATEWAY_IMAGE=ghcr.io/logicplanes/kaeso-gateway:0.1.0 pnpm stack:up
```

## Einen zweiten Stack betreiben

Ein zweiter Klon bekommt einen eigenen Stack, wenn er einen anderen Projektnamen und andere Ports hat. `KAESO_STACK_PROJECT` benennt das Compose-Projekt (standardmäßig `kaeso-dev`) und danach das Image des Gateways; `pnpm stack:up`, `pnpm seed` und `pnpm stack:down` arbeiten dann nur mit diesem Projekt. Im zweiten Klon:

```bash
export KAESO_STACK_PROJECT=kaeso-two KAESO_PUBLIC_URL=http://localhost:3001
export KAESO_GATEWAY_PORT=18180 KAESO_IDP_PORT=18191 KAESO_MAIL_PORT=58125 KAESO_MAIL_SMTP_PORT=51125
export KAESO_POSTGRES_PORT=55532 KAESO_CLICKHOUSE_PORT=58223 KAESO_VALKEY_PORT=56479
pnpm dev --port 3001
```

`pnpm seed` in derselben Shell füllt diesen Stack. Auf seine Datenbanken zeigen die Rust-Tests mit den `KAESO_TEST_…`-Variablen oben.

## Das Gateway aus dem Quellcode ausführen

Für ein Gateway, das sich in Sekunden neu starten lässt, mit Debugger oder zusätzlichem Logging, es mit Cargo gegen die Datenbanken des Stacks ausführen. Zuerst den Mock-Anbieter in einem zweiten Terminal starten, damit keine Anfrage einen echten Anbieter erreicht:

```bash
cargo run -p kaeso-testkit -- mock-provider 127.0.0.1:18090
```

Im ersten Terminal frische Datenbanken anlegen und ihre Einstellungen exportieren, beide Anbieter auf den Mock richten, eine öffentliche URL, einen Hauptschlüssel und E-Mail, die nur protokolliert wird, ergänzen und dann das Gateway starten:

```bash
export $(cargo run -q -p kaeso-testkit -- fresh-services)
export KAESO_PROVIDERS_OPENAI_BASE_URL=http://127.0.0.1:18090
export KAESO_PROVIDERS_ANTHROPIC_BASE_URL=http://127.0.0.1:18090
export KAESO_PUBLIC_URL=http://localhost:3000
export KAESO_LISTEN=127.0.0.1:8080
export KAESO_MASTER_KEYS="$(cargo run -q -p kaeso -- master-key generate)"
export KAESO_MAIL_TRANSPORT=log
cargo run -p kaeso -- serve
```

Die Konsole in einem dritten Terminal dagegen bereitstellen:

```bash
KAESO_GATEWAY_URL=http://127.0.0.1:8080 pnpm --filter kaeso-console dev
```

Dann den Einrichtungslink aus dem Log des Gateways öffnen. Um stattdessen die echten Anbieter aufzurufen, die beiden Zeilen `KAESO_PROVIDERS_…` weglassen. Die Datenbanken bleiben bestehen, bis sie im ersten Terminal gelöscht werden:

```bash
cargo run -q -p kaeso-testkit -- drop-services "$KAESO_CLICKHOUSE_DATABASE"
```

Jede Einstellung steht unter [Konfiguration](../self-hosting/configuration.md).

## Generierter Code

Zwei Dinge werden generiert statt geschrieben:

- **`openapi.json`**, das Dokument der Platform API im Wurzelverzeichnis des Repositorys, entsteht aus dem Code des Gateways. Nach einer Änderung an der Platform API neu schreiben und mit der Änderung committen:

```bash
cargo run -q -p kaeso -- openapi > openapi.json
```

- **Der API-Client und der Routenbaum der Konsole.** `pnpm --filter kaeso-console generate` schreibt die Typen des Clients aus `openapi.json` und den Routenbaum aus `apps/console/src/routes`. Die Skripte `dev`, `build`, `typecheck`, `test` und `e2e` der Konsole führen es zuerst aus, und keine der beiden Ausgaben wird committet.

Konsole und Gateway ändern sich daher gemeinsam: Eine Änderung an der Platform API aktualisiert `openapi.json` und die Konsole im selben Pull Request.

## Die Dokumentations-Website

```bash
pnpm build:packages
pnpm --filter kaeso-docs dev
```

Die Website läuft unter http://localhost:4480 und lädt neu, sobald eine Seite in `apps/docs/content` gespeichert wird. `pnpm --filter kaeso-docs check:content` prüft jede Seite, jeden Link und jede Übersetzung, ohne sie zu starten. [Dokumentation schreiben](documentation.md) beschreibt den Rest.

## Die UI-Bibliothek

```bash
pnpm build:packages
pnpm --filter kaeso-showcase dev
```

Der Showcase aller Komponenten läuft unter http://localhost:4321 und lädt neu, wenn sich der Quellcode der Bibliothek ändert. Konsole und Dokumentation lesen stattdessen den Build der Bibliothek: Nach einer Änderung an `packages/ui` oder `packages/tokens` `pnpm build:packages` ausführen und ihre Entwicklungsserver neu starten.
