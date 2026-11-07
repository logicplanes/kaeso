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

## Einrichten

Ein Befehl prüft, was das Repository braucht, installiert die Abhängigkeiten, baut die UI-Bibliothek, startet den Entwicklungs-Stack und füllt ihn mit Beispieldaten:

```bash
pnpm bootstrap
```

Er hält an, bevor er etwas ändert, wenn eine Voraussetzung fehlt oder zu alt ist, wenn Docker nicht antwortet oder wenn ein anderes Programm einen Port belegt, den der Stack braucht. Dann sagt er, wie Sie das beheben, etwa mit `corepack enable` oder der Variable, die den Port verlegt. Führen Sie ihn jederzeit erneut aus: Eine eingerichtete Installation behält ihre Daten.

Der erste Lauf baut das Gateway aus Ihrem Checkout und dauert einige Minuten; spätere Läufe nutzen den Cache von Docker. Die Beispieldaten sind die Organisation Northwind Labs mit fünf Projekten, sechs Mitgliedern, zehn Schlüsseln, Modellen, Aliassen und Limits. Einige tausend Support-E-Mails und Slack-Fragen gehen über das Gateway, die der Mock-Anbieter mit ausformulierten Antworten beantwortet. Danach folgt ein Anfrageverlauf über 70 Tage: jede App und jede Person mit eigenen Modellen, Clients und Uhrzeiten, Inhalten dort, wo das Projekt sie speichert, Treffern im Antwort-Cache, Fallbacks während ein Anbieter überlastet ist, und einem während eines Sales erreichten Limit.

| Option | Was sie bewirkt |
| --- | --- |
| `--check` | Prüft nur die Voraussetzungen, den freien Speicherplatz und die Ports |
| `--all` | Installiert auch die Browser, die Cargo-Werkzeuge und die Python-Pakete, die die [vollständigen Testsuiten](testing.md) brauchen |
| `--no-seed` | Startet den Stack ohne Beispieldaten; öffnen Sie den ausgegebenen Einrichtungslink, um Kaeso von Hand einzurichten |

Mit `KAESO_SEED_WHOLE_DAYS=1` endet der Verlauf gestern statt vor zehn Minuten, für eine Demo, die ihre Daten nach vorn verschiebt. `pnpm stack:up` und `pnpm seed` starten den Stack und die Beispieldaten einzeln ([Entwicklungs-Stack](stack.md)).

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
