---
title: Entwicklung
description: Kaeso bauen, testen und ändern, vom frischen Klon bis zum gemergten Pull Request.
order: 6
---

Diese Seiten richten sich an alle, die Kaeso selbst ändern. Alles liegt in einem Repository, [logicplanes/kaeso](https://github.com/logicplanes/kaeso), und erscheint gemeinsam unter einer Version.

| Pfad | Enthält | Gebaut mit |
| --- | --- | --- |
| `crates/` | Das Gateway, den Updater, KPP (`kaeso-kpp`, `kaeso-codecs`), den Server für die SDK-Konformität und die Aufgaben des Repositorys (`xtask`) | Rust, Cargo |
| `apps/console` | Die Web-Konsole | TypeScript, React |
| `apps/docs` | Diese Dokumentation | Markdown, TypeScript |
| `apps/showcase` | Der Showcase der UI-Bibliothek unter ui.kaeso.app | Astro, React |
| `apps/mcp-views` | Die interaktiven Ansichten, die der MCP-Server von Kaeso in Clients mit MCP Apps zeigt, ins Gateway eingebaut | TypeScript, React |
| `packages/ui`, `packages/tokens` | Die Komponentenbibliothek und die Design-Tokens, aus denen jede Oberfläche gebaut ist | TypeScript, React |
| `e2e/` | Die offiziellen Python-SDKs gegen KPP, das Gateway, sein MCP-Portal, eine frische Installation und das Helm-Chart | Python |
| `fuzz/` | Fuzz-Targets für jeden KPP-Decoder | Rust |
| `deploy/` | Die Dockerfiles, die `compose.yaml` für den Selbstbetrieb, das Helm-Chart, die Plattformvorlagen und den Entwicklungs-Stack | Docker, Helm |
| `plugins/` | Die Plugins für Claude Code und Cursor zum MCP-Server von Kaeso | JSON, Markdown |
| `openapi.json` | Das Dokument der Platform API, aus dem Code des Gateways erzeugt | |

Cargo baut den Rust-Workspace und pnpm den TypeScript-Workspace, beide vom Wurzelverzeichnis des Repositorys aus.

## Nächste Schritte

- [Lokale Einrichtung](local-setup.md): Kommen Sie vom frischen Klon zur Konsole mit Beispieldaten und bestandenen Tests.
- [Mitwirken](contributing.md): Melden Sie ein Problem, schlagen Sie eine Änderung vor und bringen Sie Ihren Pull Request zum Merge.
- [Entwicklungs-Stack](stack.md): Ändern Sie Ports, betreiben Sie einen zweiten Stack oder starten Sie das Gateway aus dem Quellcode.
- [Tests](testing.md): Führen Sie die Prüfungen aus, die eine Änderung bestehen muss, und finden Sie die Suite für Ihren Code.
- [Gateway-Benchmark](benchmark.md): Messen Sie die Zeit und den Speicher, die Kaeso hinzufügt, von einzelnen Anfragen bis zu Tausenden Streams.
- [Übersetzungen](translations.md): Schreiben Sie jedes Wort auf Englisch, Deutsch und vereinfachtem Chinesisch.
- [Dokumentation schreiben](documentation.md): Schreiben und übersetzen Sie die Seiten dieser Website.
- [Architektur](architecture/index.md): Erfahren Sie, wie Gateway, Konsole, Dokumentation und Updater gebaut sind.
- [KPP-Interna](kpp/index.md): Ändern Sie die Codecs, die zwischen den API-Formaten übersetzen.
- [Releases](releasing.md): Versionieren, veröffentlichen und verteilen Sie ein Release.
