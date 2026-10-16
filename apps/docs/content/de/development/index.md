---
title: Entwicklung
description: Wie der Quellcode von Kaeso aufgebaut ist und wie man ihn baut, testet und ändert.
order: 6
---

Kaeso wird offen in einem einzigen Repository entwickelt, [github.com/logicplanes/kaeso](https://github.com/logicplanes/kaeso): das Gateway, KPP, die Konsole, die UI-Bibliothek und diese Dokumentation, gemeinsam unter einer Versionsnummer veröffentlicht. Diese Seiten richten sich an alle, die an Kaeso selbst arbeiten.

Neu hier? [Lokale Einrichtung](local-setup.md) führt von einem frischen Klon zu einer laufenden Installation mit Beispieldaten und bestandenen Tests.

## Das Repository

| Pfad | Was er enthält | Gebaut mit |
| --- | --- | --- |
| `crates/` | Der Cargo-Workspace: das Gateway (`kaeso` und die Crates `kaeso-*`), KPP (`kaeso-kpp`, `kaeso-codecs`), der SDK-Konformitätsserver (`kpp-conformance`) und Repository-Aufgaben (`xtask`) | Rust |
| `apps/console` | Die Webkonsole | TypeScript, React |
| `apps/docs` | Diese Dokumentation, bereitgestellt unter docs.kaeso.app | Markdown, TypeScript |
| `apps/showcase` | Der Showcase der UI-Bibliothek, bereitgestellt unter [ui.kaeso.app](https://ui.kaeso.app) | Astro, React |
| `packages/ui`, `packages/tokens` | Die Komponentenbibliothek und die Design-Tokens, aus denen jede Oberfläche gebaut ist | TypeScript, React |
| `e2e/` | End-to-End-Tests mit den offiziellen Python-SDKs von OpenAI und Anthropic sowie der Installationstest | Python |
| `fuzz/` | Fuzz-Targets für jeden KPP-Decoder | Rust |
| `deploy/` | Die Dockerfiles, die `compose.yaml` für den Produktivbetrieb und der Entwicklungs-Stack | Docker |
| `openapi.json` | Das Dokument der Platform API, aus dem Code des Gateways generiert | |

Cargo baut den Rust-Workspace und pnpm den TypeScript-Workspace (`pnpm-workspace.yaml`); beide laufen vom Wurzelverzeichnis des Repositorys aus.

## Was ein Release ausliefert

| Artefakt | Gebaut aus |
| --- | --- |
| Das Image `ghcr.io/logicplanes/kaeso-gateway` | `deploy/gateway.Dockerfile` |
| Das Image `ghcr.io/logicplanes/kaeso-console` | `deploy/console.Dockerfile` |
| `compose.yaml`, an das GitHub-Release angehängt | `deploy/compose.yaml`, mit beiden Images des Releases festgelegt |
| Die Crates `kaeso-kpp` und `kaeso-codecs` auf crates.io | `crates/kaeso-kpp`, `crates/kaeso-codecs` |
| Die npm-Pakete `@logicplanes/kaeso-ui` und `@logicplanes/kaeso-tokens` | `packages/ui`, `packages/tokens` |

Die Dokumentation und der Showcase werden als Websites veröffentlicht. [Releases](releasing.md) beschreibt, wie ein Release entsteht.

## Seiten

| Seite | Was sie behandelt |
| --- | --- |
| [Lokale Einrichtung](local-setup.md) | Voraussetzungen, der Entwicklungs-Stack, Beispieldaten und wie jeder Teil aus dem Quellcode läuft |
| [Architektur](architecture/index.md) | Wie die Teile zusammenpassen, das Gateway, die Konsole, die Dokumentations-Website und die E-Mails |
| [KPP-Interna](kpp/index.md) | Das kanonische Format, die Codecs und jedes Wire-Format Feld für Feld |
| [Tests](testing.md) | Jede Testsuite, wie man sie ausführt und was bestehen muss, bevor eine Änderung fertig ist |
| [Übersetzungen](translations.md) | Wie die Konsole, die UI-Bibliothek, die E-Mails und die Dokumentation Englisch, Deutsch und Chinesisch sprechen |
| [Dokumentation schreiben](documentation.md) | Wie die Seiten dieser Website geschrieben, verlinkt und übersetzt werden |
| [Abhängigkeiten und Lizenzen](dependencies.md) | Jede Abhängigkeit, warum es sie gibt, und die Lizenzrichtlinie |
| [Performance](performance.md) | Bundle-Budgets, Codec-Benchmarks und Fuzzing-Ergebnisse |
| [Releases](releasing.md) | Versionen, der Release-Workflow und was er veröffentlicht |
| [Mitwirken](contributing.md) | Issues, Pull Requests und die Regeln, denen jede Änderung folgt |
