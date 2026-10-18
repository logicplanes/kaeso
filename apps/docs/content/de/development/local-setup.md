---
title: Lokale Einrichtung
description: Kommen Sie vom frischen Klon zur Konsole mit Beispieldaten und bestandenen Tests.
order: 1
---

Klonen Sie Kaeso, starten Sie den Entwicklungs-Stack, füllen Sie ihn mit Beispieldaten und führen Sie die Tests aus. Jeder Befehl läuft im Wurzelverzeichnis des Repositorys.

## Bevor Sie beginnen

- [Git](https://git-scm.com).
- [Node.js](https://nodejs.org) 24, die Version aus `.node-version`.
- [rustup](https://rustup.rs). Es installiert die in `rust-toolchain.toml` festgelegte Rust-Toolchain beim ersten `cargo`-Aufruf im Repository.
- Eine Docker-Engine mit Compose 2.24 oder neuer: Docker Engine unter Linux oder [Colima unter macOS](stack.md#docker-mit-colima-unter-macos-betreiben).
- [Python](https://www.python.org) 3.13, nur für die [End-to-End-Tests](testing.md#end-to-end-tests).

## Corepack einschalten

Corepack gehört zu Node.js und stellt genau die pnpm-Version bereit, die das Repository festlegt:

```bash
corepack enable
```

`pnpm --version` gibt jetzt die Version aus, die in `package.json` steht.

## Code holen

```bash
git clone https://github.com/logicplanes/kaeso
cd kaeso
```

## Abhängigkeiten installieren

Installieren Sie Konsole, Dokumentation, Showcase und die Bibliotheken in `packages/`:

```bash
pnpm install
```

Cargo holt die Rust-Crates beim ersten Build.

## Entwicklungs-Stack starten

Bauen Sie das Gateway aus Ihrem Checkout und starten Sie es mit seinen Datenbanken und Mock-Diensten:

```bash
pnpm stack:up
```

Der erste Build kompiliert das Gateway und dauert einige Minuten; spätere Builds nutzen den Cache von Docker. Sobald alle Dienste gesund sind, gibt der Befehl einen Einrichtungslink aus, `http://localhost:3000/setup#token=…`.

## Beispieldaten anlegen

Richten Sie die Installation über die Platform API ein und füllen Sie sie mit Beispieldaten:

```bash
pnpm seed
```

Sie erhalten die Organisation Northwind Labs mit fünf Projekten, sechs Mitgliedern, Schlüsseln, Modellen, einem Alias und Limits. Der Befehl sendet außerdem einige hundert Anfragen an den Mock-Anbieter und legt einen Verlauf früherer Anfragen über 70 Tage an. Um die Installation stattdessen von Hand einzurichten, überspringen Sie diesen Schritt und öffnen den Einrichtungslink.

## Konsole öffnen

Bauen Sie die Bibliotheken und starten Sie die Konsole:

```bash
pnpm dev
```

Die Konsole läuft unter http://localhost:3000 und lädt neu, sobald Sie speichern. Melden Sie sich als `owner@example.com` an, Inhaber der Organisation und Admin der Instanz, oder als `lin@example.com`, ein Entwickler. Beide nutzen das Passwort `correct horse battery staple`. E-Mails kommen in Mailpit unter http://127.0.0.1:58025 an.

## Tests ausführen

Führen Sie die TypeScript- und die Rust-Tests aus:

```bash
pnpm test
cargo test --workspace --all-features
```

Beide laufen ohne Fehler durch. Die Rust-Tests legen eigene Datenbanken im Stack an und lassen die Beispieldaten unberührt.

## Nächste Schritte

- [Mitwirken](contributing.md): Nehmen Sie eine Änderung vor und bringen Sie Ihren Pull Request zum Merge.
- [Entwicklungs-Stack](stack.md): Ändern Sie Ports, betreiben Sie einen zweiten Stack oder starten Sie das Gateway aus dem Quellcode.
- [Tests](testing.md): Führen Sie jede Prüfung aus, die eine Änderung bestehen muss.
- [Architektur](architecture/index.md): Finden Sie sich in Gateway, Konsole und Dokumentation zurecht.
