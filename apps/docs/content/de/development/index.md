---
title: Entwicklung
description: Wie der Quellcode von Kaeso aufgebaut ist und wie man ihn baut, testet und ändert.
order: 6
---

Diese Seiten richten sich an alle, die an Kaeso selbst arbeiten. Alles liegt in einem Repository, [github.com/logicplanes/kaeso](https://github.com/logicplanes/kaeso), und wird gemeinsam unter einer Version veröffentlicht. Für einen lauffähigen Checkout mit der [lokalen Einrichtung](local-setup.md) beginnen.

## Das Repository

| Pfad | Inhalt | Gebaut mit |
| --- | --- | --- |
| `crates/` | Das Gateway (`kaeso` und die `kaeso-*`-Crates), KPP (`kaeso-kpp`, `kaeso-codecs`), der SDK-Konformitätsserver (`kpp-conformance`) und Repository-Aufgaben (`xtask`) | Rust, Cargo |
| `apps/console` | Die Web-Konsole | TypeScript, React |
| `apps/docs` | Diese Dokumentation unter docs.kaeso.app | Markdown, TypeScript |
| `apps/showcase` | Der Showcase der UI-Bibliothek unter ui.kaeso.app: jede Komponente in jedem Zustand | Astro, React |
| `packages/ui`, `packages/tokens` | Die Komponentenbibliothek und die Design-Tokens, aus denen jede Oberfläche gebaut ist | TypeScript, React |
| `e2e/` | Die offiziellen Python-SDKs von OpenAI und Anthropic gegen KPP, das Gateway und eine frische Installation | Python |
| `fuzz/` | Fuzz-Targets für jeden KPP-Decoder | Rust |
| `deploy/` | Die Dockerfiles, die produktive `compose.yaml` und der Entwicklungs-Stack | Docker |
| `openapi.json` | Das Dokument der Platform API, aus dem Code des Gateways erzeugt | |

Cargo baut den Rust-Workspace, pnpm den TypeScript-Workspace (`pnpm-workspace.yaml`). Beide laufen vom Wurzelverzeichnis des Repositorys aus.

## Was ein Release ausliefert

| Artefakt | Gebaut aus |
| --- | --- |
| Die Images `ghcr.io/logicplanes/kaeso-gateway` und `ghcr.io/logicplanes/kaeso-console` | `deploy/gateway.Dockerfile`, `deploy/console.Dockerfile` |
| `compose.yaml`, `kaeso.example.toml` und `env.example`, am GitHub-Release angehängt | `deploy/compose.yaml` mit beiden Images auf das Release festgelegt, `deploy/kaeso.example.toml`, `deploy/.env.example` |
| Die Crates `kaeso-kpp` und `kaeso-codecs` auf crates.io | `crates/kaeso-kpp`, `crates/kaeso-codecs` |
| Die npm-Pakete `@logicplanes/kaeso-ui` und `@logicplanes/kaeso-tokens` | `packages/ui`, `packages/tokens` |

Dokumentation und Showcase werden als statische Websites aus dem Tag des Releases bereitgestellt. [Releases](releasing.md) beschreibt den ganzen Ablauf.

## Seiten

| Seite | Inhalt |
| --- | --- |
| [Mitwirken](contributing.md) | Issues, Pull Requests und die Regeln, denen jede Änderung folgt |
| [Lokale Einrichtung](local-setup.md) | Voraussetzungen, der Entwicklungs-Stack, Beispieldaten und jeder Teil aus dem Quellcode |
| [Architektur](architecture/index.md) | Wie Gateway, Konsole, Dokumentations-Website und E-Mails zusammenspielen |
| [KPP-Interna](kpp/index.md) | Das kanonische Format, die Codecs und jedes Wire-Format Feld für Feld |
| [Tests](testing.md) | Jede Testsuite, wie man sie ausführt und was CI verlangt |
| [Übersetzungen](translations.md) | Wie jede Oberfläche Englisch, Deutsch und vereinfachtes Chinesisch spricht, und das Glossar |
| [Dokumentation schreiben](documentation.md) | Wie Seiten dieser Website geschrieben, verlinkt und übersetzt werden |
| [Abhängigkeiten und Lizenzen](dependencies.md) | Jede direkte Abhängigkeit, warum es sie gibt, und die Lizenzrichtlinie |
| [Releases](releasing.md) | Versionen, das Changelog, der Release-Workflow und was er veröffentlicht |
