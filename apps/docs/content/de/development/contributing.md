---
title: Mitwirken
description: Issues melden, Änderungen vorschlagen und die Regeln, denen jede Änderung an Kaeso folgt.
order: 10
---

Issues sind im [Repository](https://github.com/logicplanes/kaeso/issues) willkommen: Fehler, Fragen und Ideen. Pull Requests von externen Mitwirkenden prüft der Maintainer nach eigenem Ermessen; für alles, was größer ist als eine kleine Korrektur, zuerst ein Issue eröffnen, damit sich der Ansatz abstimmen lässt. Sicherheitsprobleme vertraulich über die [Security Advisories](https://github.com/logicplanes/kaeso/security/advisories/new) des Repositorys melden, nie in einem öffentlichen Issue.

## Eine Änderung vornehmen

1. **Von `dev` abzweigen**, dem Standard-Branch. `prod` bewegt sich nur durch Releases.
2. **Erst einen fehlschlagenden Test schreiben, dann den Code.** Jede Regel hat einen Test, und jede Fehlerbehebung beginnt mit einem Test, der den Fehler zeigt.
3. **Alles ändern, was die Änderung berührt, im selben Pull Request.** Gateway und Konsole ändern sich gemeinsam, wenn sich die Platform API ändert, und `openapi.json` wird neu geschrieben. Eine KPP-Änderung beginnt mit einer [Fixture](kpp/fixtures.md) und aktualisiert die Mapping-Seite des Dialekts. Eine neue Abhängigkeit bekommt eine Zeile unter [Abhängigkeiten](dependencies.md). Eine Änderung an Rechten, Rollen oder Einstellungen ändert einen Referenz-Snapshot; dessen Diff prüfen und die passende Dokumentationsseite danach aktualisieren.
4. **Die Dokumentation aktualisieren.** Wenn Personen, die Kaeso nutzen, betreiben oder integrieren, die Änderung bemerken, ändern sich die Seiten in `apps/docs/content` mit, auf Englisch, Deutsch und vereinfachtem Chinesisch ([Dokumentation schreiben](documentation.md)).
5. **Eine Zeile in `CHANGELOG.md` ergänzen**, unter `[Unreleased]`, für jede Änderung, die Personen bemerken werden ([das Changelog](releasing.md#das-changelog)).
6. **Die Prüfungen ausführen.** Alles unter [Bevor eine Änderung fertig ist](testing.md#bevor-eine-änderung-fertig-ist) besteht, und jede geänderte Oberfläche ist in beiden Themes, in Desktop- und Telefonbreite und in allen drei Sprachen angesehen.
7. **Einen Pull Request nach `dev` eröffnen.** Ihn auf eine Änderung beschränken, die Vorlage ausfüllen und sagen, was sich geändert hat und warum.

## Regeln für jede Änderung

- **Keine Code-Kommentare.** Keine `//`-, `///`-, `/* */`-, JSDoc- oder JSX-Kommentare, kein `--` in SQL, keine `#`-Kommentare in TOML, YAML, Python, Shell, Dockerfiles oder der Caddyfile und keine in den Codeblöcken einer Seite. Code erklärt sich durch Namen, Typen, kleine Funktionen und Tests; Erklärungen gehören in diese Dokumentation.
- **Tests für jede Änderung**, die prüfen, was eine Person oder eine App sieht. Coverage-Schwellen werden nie gesenkt; steigt die Coverage, werden sie angehoben.
- **Oberflächen nutzen nur `@logicplanes/kaeso-ui`**, mit `className` nur für Layout, und jedes Wort in drei Sprachen ([Übersetzungen](translations.md)).
- **Keine Secrets oder personenbezogenen Daten** im Repository: nur Platzhalter wie `kaeso.example.com`, `sk-kso-…` und `owner@example.com`.
- **Keine Pläne im Repository.** Roadmaps, To-do-Listen und offene Fragen gehören in Issues.
- **pnpm ist der einzige JavaScript-Paketmanager.** Eine Abhängigkeit in exakter Version mit `pnpm --filter <package> add --save-exact <name>` hinzufügen.

Die eigenen Regeln des Gateways, etwa keine Panics in Bibliothekscode, keine Gleitkommazahlen für Geld und genau ein Recht pro Endpunkt, stehen unter [Gateway](architecture/gateway.md#regeln), die von KPP unter [KPP-Interna](kpp/index.md#entwurfsprinzipien).

## Agenten

Jeder Teil des Repositorys hat eine `AGENTS.md` mit seinen Regeln und Prüfungen, für Coding-Agenten und für Menschen: das Wurzelverzeichnis, `crates`, `packages`, `apps/console`, `apps/docs` und `apps/showcase`. Sie sagen dasselbe wie diese Seiten, nur kürzer. `packages/ui/AGENTS.md` enthält die Regeln für Apps, die mit der UI-Bibliothek gebaut werden, und wird mit ihrem npm-Paket ausgeliefert.

## Lizenz

Wer beiträgt, stimmt zu, dass die eigenen Beiträge unter der [MIT-Lizenz](https://github.com/logicplanes/kaeso/blob/dev/LICENSE) lizenziert werden.
