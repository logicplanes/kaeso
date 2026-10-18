---
title: Mitwirken
description: Issues melden, Änderungen vorschlagen und die Regeln, denen jede Änderung an Kaeso folgt.
order: 10
---

Ideen und Fragen gehören in die [Discussions](https://github.com/logicplanes/kaeso/discussions), Fehler und konkrete Feature-Wünsche in die [Issues](https://github.com/logicplanes/kaeso/issues). Pull Requests von außen prüft der Maintainer nach eigenem Ermessen; für alles, was über eine kleine Korrektur hinausgeht, zuerst ein Issue eröffnen, damit sich der Ansatz abstimmen lässt. Alle Beteiligten folgen dem [Verhaltenskodex](https://github.com/logicplanes/kaeso/blob/dev/CODE_OF_CONDUCT.md).

## Discussions und Issues

In den Discussions wird eine Idee besprochen, bevor etwas entschieden ist: **Ideas** für ein gewünschtes Feature, **Q&A** für Fragen zu Installation, Betrieb und Nutzung, **Show and tell** für etwas, das mit Kaeso gebaut wurde. Eine abgestimmte Idee wird zum Feature-Request.

Für Issues gibt es zwei Vorlagen:

| Vorlage | Für |
| --- | --- |
| Bug report | Kaeso tut etwas, das es nicht tun sollte, oder tut nicht, was es tun sollte. Die Vorlage fragt nach Version, Bereich, dem Geschehenen, der kleinsten Reproduktion und der Umgebung. Bei allem Sichtbaren fragt sie außerdem nach Sprache, Theme, Breite und Browser |
| Feature request | Eine neue Fähigkeit, eine Änderung an einer bestehenden oder etwas, das die Dokumentation erklären sollte. Für jedes betroffene Wire-Feld die Dokumentation oder das OpenAPI-Schema des Anbieters verlinken |

API-Schlüssel, Anbieter-Schlüssel, Sitzungscookies und personenbezogene Daten aus allem entfernen, was eingefügt wird.

> [!IMPORTANT]
> Sicherheitsprobleme vertraulich über die [Security Advisories](https://github.com/logicplanes/kaeso/security/advisories/new) des Repositorys melden, nie in einem öffentlichen Issue. [`SECURITY.md`](https://github.com/logicplanes/kaeso/blob/dev/SECURITY.md) nennt, was eine Meldung enthalten sollte und was danach geschieht.

## Eine Änderung vornehmen

1. **Von `dev` abzweigen**, dem Standard-Branch. `prod` hält das neueste Release, und nur der Maintainer pusht darauf ([Branches](releasing.md#branches)).
2. **Erst einen fehlschlagenden Test schreiben, dann den Code.** Jede Fehlerbehebung beginnt mit einem Test, der den Fehler zeigt.
3. **Alles ändern, was die Änderung berührt, im selben Pull Request.** Eine Änderung an der Platform API aktualisiert Gateway, Konsole und `openapi.json` gemeinsam. Eine KPP-Änderung beginnt mit einer [Fixture](kpp/fixtures.md) und aktualisiert die Mapping-Seite des Dialekts. Eine neue Abhängigkeit bekommt eine Zeile unter [Abhängigkeiten](dependencies.md). Eine Änderung an Rechten, Rollen oder Einstellungen ändert einen [Referenz-Snapshot](testing.md#snapshots-und-referenzen); dessen Diff prüfen und die passende Seite aktualisieren.
4. **Die Dokumentation aktualisieren**, in `apps/docs/content` auf Englisch, Deutsch und vereinfachtem Chinesisch, wenn Personen, die Kaeso nutzen, betreiben oder integrieren, die Änderung bemerken ([Dokumentation schreiben](documentation.md)).
5. **Den Titel als Zeile des Changelogs schreiben.** Der Release-Workflow schreibt das Changelog aus den gemergten Pull Requests: Der Titel wird zur Zeile, ein Satz, der sagt, was sich für alle geändert hat, die Kaeso nutzen, betreiben oder darauf aufbauen, etwa „Keep dialogs centered and resize them smoothly when their content changes“. `CHANGELOG.md` nicht ändern. Der Workflow **Labels** setzt die Bereichs-Labels und `internal` aus den geänderten Dateien, der Maintainer den Typ, etwa `fixed` ([Labels](releasing.md#labels)).
6. **Die Prüfungen ausführen**, die unter [Bevor eine Änderung fertig ist](testing.md#bevor-eine-änderung-fertig-ist) stehen, und jede geänderte Oberfläche in beiden Themes, in Desktop- und Telefonbreite und in allen drei Sprachen ansehen.
7. **Einen Pull Request nach `dev` eröffnen.** Ihn auf eine Änderung beschränken. Die Vorlage fragt, was sich geändert hat und warum, wie es getestet wurde, und enthält eine Checkliste der Schritte oben.

### Wie Änderungen nach dev gelangen

Jede Änderung gelangt über einen Pull Request nach `dev`. Gemergt werden kann er, sobald die Pflichtprüfung **CI** grün ist ([Continuous Integration](testing.md#continuous-integration)), und zwar als ein einziger gesquashter Commit; Merge-Commits und Rebase-Merges sind abgeschaltet. Der Titel des Pull Requests wird zur Betreffzeile des Commits, deshalb sagen er und jede Commit-Nachricht, was sich geändert hat und warum. Der Maintainer kann diese Regeln umgehen.

### Gestapelte Pull Requests

Baut eine Änderung auf einer anderen auf, die noch nicht gemergt ist, werden beide als Stapel mit den Stacked Pull Requests von GitHub eröffnet statt als ein großer Pull Request. Der Befehl `gh stack` kommt aus der Erweiterung `github/gh-stack` (`gh extension install github/gh-stack`):

```bash
gh stack init
gh stack add store-tokens
gh stack submit
```

`gh stack init` beginnt einen Stapel auf `dev`, `gh stack add` legt einen Branch über den aktuellen, und `gh stack submit` pusht jeden Branch und eröffnet für jeden einen Pull Request auf dem Branch darunter. `gh stack rebase` bringt den Stapel auf Stand, wenn sich ein Branch darunter ändert. Die Pull Requests werden von unten nach oben geprüft und gemergt, jeder mit eigener grüner CI.

## Regeln für jede Änderung

- **Keine Code-Kommentare, nirgends.** Keine `//`-, `///`-, `//!`-, `/* */`-, JSDoc- oder JSX-Kommentare; kein `--` in SQL; keine `#`-Kommentare in TOML, YAML, Python, Shell, Dockerfiles, `.dockerignore`, der Caddyfile, `.env.example` oder den Ignore- und Attributes-Dateien; kein `<!-- -->` in HTML, Astro oder Markdown; keine in den Codeblöcken einer Seite. Namen, Typen, kleine Funktionen und Tests erklären den Code, diese Dokumentation erklärt den Rest. Der CI-Job **No comments** setzt das durch.
- **Tests für jede Änderung**, die prüfen, was eine Person oder eine App sieht, deterministisch, mit injizierbaren Uhren und ohne Wartezeiten. Coverage-Schwellen werden nie gesenkt; steigt die Coverage, steigen sie mit.
- **Oberflächen nutzen nur `@logicplanes/kaeso-ui`**, `className` nur für Layout, und jedes Wort gibt es in drei Sprachen ([Übersetzungen](translations.md)).
- **Nur MIT-kompatible Abhängigkeiten**, geprüft von `cargo deny` und `pnpm licenses:check` ([Lizenzrichtlinie](dependencies.md#lizenzrichtlinie)).
- **Keine Secrets oder personenbezogenen Daten** im Repository, sondern Platzhalter wie `kaeso.example.com`, `sk-kso-…` und `owner@example.com`.
- **Keine Pläne im Repository.** Roadmaps, To-do-Listen und offene Fragen gehören in Issues.
- **Nur pnpm** für JavaScript; `pnpm dlx` führt ein einmaliges Werkzeug aus. Abhängigkeiten in exakter Version mit `pnpm --filter <package> add --save-exact <name>` hinzufügen. Cargo baut den Rust-Code. Lockfiles ändern sich nur über ihr eigenes Werkzeug.

Das Gateway hat eigene Regeln, etwa keine Panics in Bibliothekscode, keine Gleitkommazahlen für Geld und genau ein Recht pro Endpunkt ([Gateway](architecture/gateway.md#regeln)); KPP ebenso ([Entwurfsprinzipien](kpp/index.md#entwurfsprinzipien)).

## AGENTS.md-Dateien

Das Wurzelverzeichnis, `crates`, `packages`, `apps/console`, `apps/docs` und `apps/showcase` haben je eine `AGENTS.md` mit den Regeln und Prüfungen dieses Teils, für Coding-Agenten und für Menschen. Sie fassen diese Seiten zusammen. `packages/ui/AGENTS.md` enthält die Regeln für Apps, die mit der UI-Bibliothek gebaut werden, und wird mit ihrem npm-Paket ausgeliefert.

## Lizenz

Wer beiträgt, stimmt zu, dass die eigenen Beiträge unter der [MIT-Lizenz](https://github.com/logicplanes/kaeso/blob/dev/LICENSE) lizenziert werden.
