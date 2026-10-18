---
title: Mitwirken
description: Melden Sie ein Problem, schlagen Sie eine Änderung vor und bringen Sie Ihren Pull Request zum Merge.
order: 2
---

Stellen Sie Fragen und teilen Sie Ideen in den [Discussions](https://github.com/logicplanes/kaeso/discussions), und melden Sie Fehler und konkrete Feature-Wünsche als [Issues](https://github.com/logicplanes/kaeso/issues). Für alles, was über eine kleine Korrektur hinausgeht, eröffnen Sie zuerst ein Issue. So einigen Sie sich mit dem Maintainer auf den Ansatz, bevor Sie Code schreiben. Alle Beteiligten folgen dem [Verhaltenskodex](https://github.com/logicplanes/kaeso/blob/dev/CODE_OF_CONDUCT.md).

## Diskussion beginnen

Besprechen Sie eine Idee in den Discussions, bevor etwas entschieden ist. Nutzen Sie **Ideas** für ein Feature, das Sie sich wünschen, **Q&A** für Fragen zu Installation, Betrieb und Nutzung von Kaeso und **Show and tell** für etwas, das Sie gebaut haben. Eine Idee, der der Maintainer zustimmt, wird zum Feature-Wunsch.

## Issue eröffnen

Wählen Sie die passende Vorlage:

| Vorlage | Für |
| --- | --- |
| Bug report | Kaeso tut etwas, das es nicht tun sollte, oder tut nicht, was es tun sollte. Die Vorlage fragt nach Version, Bereich, dem Geschehenen, der kleinsten Reproduktion und dem Betrieb von Kaeso. Bei allem Sichtbaren fragt sie außerdem nach Sprache, Theme, Breite und Browser |
| Feature request | Eine neue Fähigkeit, eine Änderung an einer bestehenden oder etwas, das die Dokumentation erklären sollte. Verlinken Sie für jedes betroffene Wire-Feld die Dokumentation oder das OpenAPI-Schema des Anbieters |

Entfernen Sie API-Schlüssel, Anbieter-Schlüssel, Sitzungscookies und personenbezogene Daten aus allem, was Sie einfügen.

> [!IMPORTANT]
> Melden Sie Sicherheitsprobleme vertraulich über die [Security Advisories](https://github.com/logicplanes/kaeso/security/advisories/new) des Repositorys, nie in einem öffentlichen Issue. [`SECURITY.md`](https://github.com/logicplanes/kaeso/blob/dev/SECURITY.md) nennt, was eine Meldung enthalten sollte und was danach geschieht.

## Änderung vornehmen

1. Zweigen Sie von `dev` ab, dem Standard-Branch.
2. Schreiben Sie zuerst einen fehlschlagenden Test. Eine Fehlerbehebung beginnt mit einem Test, der den Fehler zeigt.
3. Ändern Sie den Code, bis der Test besteht.
4. Ändern Sie alles andere, was die Änderung berührt, im selben Pull Request. Eine Änderung an der Platform API aktualisiert Gateway, Konsole und `openapi.json` gemeinsam. Eine KPP-Änderung beginnt mit einer [Fixture](kpp/fixtures.md) und aktualisiert die Mapping-Seite des Dialekts.
5. Aktualisieren Sie die Dokumentation auf Englisch, Deutsch und vereinfachtem Chinesisch, wenn Personen, die Kaeso nutzen, betreiben oder integrieren, die Änderung bemerken ([Dokumentation schreiben](documentation.md)).
6. Führen Sie die Prüfungen unter [Bevor eine Änderung fertig ist](testing.md#bevor-eine-änderung-fertig-ist) aus. Sehen Sie sich jede geänderte Oberfläche in Hell und Dunkel, in Desktop- und Telefonbreite und in allen drei Sprachen an.
7. Öffnen Sie einen Pull Request nach `dev`, der genau diese eine Änderung enthält. Die Vorlage fragt, was sich warum geändert hat, wie Sie es getestet haben, und nach ihrer Checkliste.

Schreiben Sie nach einer Änderung an der Platform API `openapi.json` neu und committen Sie die Datei mit der Änderung:

```bash
cargo run -q -p kaeso -- openapi > openapi.json
```

Eine Änderung an Rechten, Rollen oder Einstellungen ändert einen [Referenz-Snapshot](testing.md#snapshots-und-referenzen). Prüfen Sie seinen Diff und aktualisieren Sie die Seite, zu der er gehört.

## Titel schreiben

Der Titel wird zur Zeile des Pull Requests im Changelog. Schreiben Sie deshalb einen Satz, der sagt, was sich für Personen ändert, die Kaeso nutzen, betreiben oder darauf aufbauen. Zum Beispiel: „Keep dialogs centered and resize them smoothly when their content changes“. Ändern Sie `CHANGELOG.md` nicht: Der Release-Workflow schreibt die Datei aus den gemergten Pull Requests.

Der Workflow **Labels** setzt die Bereichs-Labels, etwa `area: console`, und `internal` anhand der Dateien, die Sie ändern. Der Maintainer setzt ein Typ-Label wie `fixed`, wenn das erste Wort des Titels nicht den richtigen Abschnitt wählt ([Labels](releasing.md#labels)).

## Merge erreichen

Ihr Pull Request kann gemergt werden, sobald die erforderliche Prüfung **CI** grün ist ([Continuous Integration](testing.md#continuous-integration)). Er wird als ein einziger gesquashter Commit gemergt, dessen Betreff der Titel ist. Lassen Sie deshalb jede Commit-Nachricht sagen, was sich warum geändert hat.

Mit Ihrem Beitrag stimmen Sie zu, dass er unter der [MIT-Lizenz](https://github.com/logicplanes/kaeso/blob/dev/LICENSE) steht.

## Abhängige Pull Requests stapeln

Baut eine Änderung auf einer anderen auf, die noch nicht gemergt ist, öffnen Sie beide als Stack statt als einen großen Pull Request. Installieren Sie den Befehl `gh stack` einmalig:

```bash
gh extension install github/gh-stack
```

Beginnen Sie einen Stack auf `dev`, fügen Sie für jede Änderung einen Branch auf dem darunterliegenden hinzu und öffnen Sie die Pull Requests:

```bash
gh stack init
gh stack add store-tokens
gh stack submit
```

`gh stack submit` pusht jeden Branch und öffnet für jeden einen Pull Request, der auf dem Branch darunter basiert. Ändert sich ein Branch weiter unten, bringt `gh stack rebase` die übrigen auf den neuesten Stand. Die Pull Requests werden von unten nach oben geprüft und gemergt, jeder mit eigener grüner CI.

## Hausregeln befolgen

Jede Änderung folgt diesen Regeln, und CI prüft die meisten davon:

- **Keine Code-Kommentare, nirgends.** Namen, Typen, kleine Funktionen und Tests erklären den Code, und diese Dokumentation erklärt den Rest. Das gilt auch für `#` in Konfigurationsdateien, `<!-- -->` in Markdown und Kommentare in den Codeblöcken einer Seite. Der Job **No comments** setzt das durch.
- **Tests prüfen, was eine Person oder eine App sieht.** Sie sind deterministisch, mit injizierten Uhren und ohne Sleeps. Coverage-Schwellen steigen nur.
- **Oberflächen nutzen nur `@logicplanes/kaeso-ui`**, mit `className` nur für Layout, und jedes Wort gibt es in drei Sprachen ([Übersetzungen](translations.md)).
- **Nur pnpm** für JavaScript, und `pnpm dlx` für ein einmaliges Werkzeug. Cargo baut den Rust-Code, und Lockfiles ändern sich nur über ihr Werkzeug.
- **Keine Geheimnisse und keine personenbezogenen Daten** im Repository. Nutzen Sie Platzhalter wie `kaeso.example.com`, `sk-kso-…` und `owner@example.com`.
- **Keine Pläne im Repository.** Roadmaps, To-do-Listen und offene Fragen gehören in Issues.

Das Gateway hat eigene Regeln, etwa keine Panics und keine Gleitkommazahlen für Geld ([Regeln des Gateways](architecture/gateway.md#regeln)). KPP ebenso ([Entwurfsprinzipien](kpp/index.md#entwurfsprinzipien)).

Das Wurzelverzeichnis des Repositorys und jedes von `crates`, `packages`, `apps/console`, `apps/docs` und `apps/showcase` haben eine `AGENTS.md`. Jede fasst die Regeln und Prüfungen ihres Teils zusammen, für Coding-Agents und für Menschen.

## Abhängigkeit hinzufügen

Fügen Sie eine JavaScript-Abhängigkeit mit `pnpm --filter <package> add --save-exact <name>` in einer exakten Version hinzu und eine Rust-Abhängigkeit mit `cargo add`. Begründen Sie in einer Zeile der Beschreibung des Pull Requests, warum sie nötig ist.

Kaeso steht unter der MIT-Lizenz, und alles, was Kaeso ausliefert, hängt nur von freizügig lizenziertem Code ab. Code unter GPL, LGPL und AGPL ist nie erlaubt. Zwei Prüfungen setzen die Richtlinie durch:

| Was ausgeliefert wird | Erlaubte Lizenzen | Geprüft von |
| --- | --- | --- |
| Das Gateway und der Updater | Die Liste in `deny.toml`: MIT, MIT-0, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode und Zlib | `cargo deny check` |
| `kaeso-kpp`, `kaeso-codecs` | Dieselbe Liste, ohne Ausnahmen | `cargo deny --manifest-path crates/kaeso-codecs/Cargo.toml check` |
| Die UI-Bibliothek, die Konsole und die Dokumentation | MIT, ISC, BSD-2-Clause, BSD-3-Clause, Apache-2.0 und 0BSD | `pnpm licenses:check`, nach `pnpm build` |

`cargo deny check` lehnt außerdem Crates mit Sicherheitshinweisen ab, zurückgezogene Crates, Wildcard-Versionen und Crates aus unbekannten Registries oder Git-Repositorys. Einige Crates, die das Gateway nur über andere Crates erreicht, haben eine Ausnahme für eine andere freizügige Lizenz; `deny.toml` nennt jede davon. Werkzeuge, die nur in Tests oder Builds laufen, etwa axe-core, erreichen nie ein Paket, ein Image oder eine veröffentlichte Website. Die Schriften der UI-Bibliothek, Inter und Geist Mono, stehen unter der SIL Open Font License, und `pnpm release:check` lehnt ein Paket ohne ihre Lizenzdateien ab.

Der Bibliothekscode von KPP darf nur von `serde`, `serde_json`, `bytes`, `thiserror`, `indexmap`, `base64` und `mime` abhängen, und seine Richtlinie verbietet `tokio`, `reqwest`, `hyper` und `axum`. Alles andere braucht vorher die Zustimmung des Maintainers.

## Nächste Schritte

- [Lokale Einrichtung](local-setup.md): Bringen Sie einen Checkout mit Beispieldaten zum Laufen.
- [Tests](testing.md): Finden Sie die Suite, die Ihre Änderung abdeckt.
- [Dokumentation schreiben](documentation.md): Aktualisieren Sie die Seiten, die Ihre Änderung berührt.
- [Übersetzungen](translations.md): Ergänzen Sie Deutsch und Chinesisch für neuen Text.
