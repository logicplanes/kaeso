---
title: Releases
description: Wie Versionen nummeriert werden, was ein Release veröffentlicht und wie der Maintainer eines erstellt.
order: 9
---

Kaeso wird als Ganzes veröffentlicht: Das Gateway, die Konsole, die Crates von KPP und die Pakete der UI-Bibliothek teilen sich eine Versionsnummer, und ein Release veröffentlicht sie alle. Mitwirkende erstellen nie Tags oder Releases; der Maintainer erstellt jedes Release aus `prod` mit dem Release-Workflow des Repositorys und veröffentlicht die Dokumentation mit dessen Pages-Workflow.

## Versionen

Versionen folgen [Semantic Versioning](https://semver.org). Ein Release, das von Betreibern von Kaeso einen manuellen Schritt verlangt, etwa eine Migration, die nicht von selbst laufen kann, ist ein Major-Release. Ein Pre-Release hängt ein Suffix an, etwa `0.2.0-rc.1`. Der Git-Tag eines Releases ist `v` gefolgt von seiner Version, etwa `v0.1.0`.

Gateway und Konsole eines Releases gehören immer zusammen: Die `compose.yaml` des Releases legt beide Images auf diese Version fest, und die Konsole erwartet genau die Platform API ihres Gateways.

## Branches

| Branch | Was er ist |
| --- | --- |
| `dev` | Der Standard-Branch. Jeder Pull Request geht hierhin, und der Maintainer darf direkt darauf pushen |
| `prod` | Das neueste Release oder der Commit, der gleich eines wird. Der Maintainer pusht darauf, um zu veröffentlichen, immer als Fast-Forward, und der Release-Workflow läuft nur hier |
| `hotfix/X.Y.Z` | Eine Korrektur für das neueste Release, von seinem Tag abgezweigt und auf `prod` gepusht |

`dev` und `prod` lassen sich weder löschen noch per Force-Push überschreiben, und ein `v*`-Tag lässt sich weder verschieben noch löschen. Nach jedem Release bringt der Release-Workflow das Release nach `dev`, sodass der nächste Push von `dev` auf `prod` wieder ein Fast-Forward ist.

## Was ein Release veröffentlicht

| Wo | Was |
| --- | --- |
| GitHub Releases | Das Release `vX.Y.Z` mit dem Titel „Kaeso X.Y.Z“, seinen Release Notes aus `CHANGELOG.md` und diesen Dateien: `compose.yaml`, die [Installationen](../self-hosting/installation.md) herunterladen; `kaeso.example.toml`; `env.example`; die beiden npm-Tarballs und die beiden `.crate`-Dateien |
| GitHub Container Registry | `ghcr.io/logicplanes/kaeso-gateway` und `ghcr.io/logicplanes/kaeso-console` für `linux/amd64` und `linux/arm64`, mit Build-Provenienz und einer Software-Stückliste (SBOM) |
| crates.io | `kaeso-kpp` und `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` und `@logicplanes/kaeso-tokens`, mit Provenienz |

| Image-Tag | Erhält |
| --- | --- |
| Die Version, etwa `0.2.0` oder `0.2.0-rc.1` | Jedes Release |
| `MAJOR.MINOR`, etwa `0.2`, und `latest` | Jedes Release, das kein Pre-Release ist |
| `MAJOR`, etwa `1` | Jedes Release ab `1.0.0`, das kein Pre-Release ist |

Auf npm erhält ein Release den Tag `latest` und ein Pre-Release den Tag `next`. Ein Pre-Release ist auf GitHub als solches markiert und wird nie zum neuesten Release, sodass `releases/latest/download/compose.yaml` immer auf das neueste stabile Release zeigt.

## Das Changelog

`CHANGELOG.md` im Wurzelverzeichnis des Repositorys folgt [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Jede Änderung, die Personen bemerken werden, ergänzt im selben Pull Request eine Zeile unter `[Unreleased]`: was sich geändert hat, aus Sicht von jemandem, der Kaeso nutzt, betreibt oder darauf aufbaut.

Ein Release fügt unter `## [Unreleased]` die Überschrift `## [X.Y.Z] - YYYY-MM-DD` ein, sodass die dort gesammelten Zeilen zum Abschnitt der Version werden. Am Ende der Datei vergleicht `[Unreleased]` danach mit dem neuen Tag, und ein neuer Link zeigt auf das Release der Version. Dieser Abschnitt wird zu den Release Notes. Ein Release ohne Einträge unter `[Unreleased]` wird abgelehnt, es sei denn, `CHANGELOG.md` hat schon einen Abschnitt für diese Version oder, bei einem stabilen Release, für ein Pre-Release davon.

Ein stabiles Release nach Pre-Releases seiner Version, etwa `0.2.0` nach `0.2.0-rc.1` und `0.2.0-rc.2`, fasst deren Abschnitte und alles Neue unter `[Unreleased]` im einen Abschnitt `## [0.2.0] - YYYY-MM-DD` zusammen: Die Zeilen unter jeder Überschrift, etwa `### Added`, und unter jedem Thema darin, etwa `#### Model API`, kommen zusammen, die neuesten zuerst, und die Abschnitte und Links der Pre-Releases entfallen. `scripts/changelog.mjs` nimmt diese Änderungen vor, und die Release Notes auf GitHub sind der Abschnitt, den es schreibt. Seine Release Notes nennen dann alles, was die Version seit dem letzten stabilen Release ändert.

## Ein Release erstellen

1. **`dev` vorbereiten.** Jede Prüfung der [Definition of Done](testing.md#bevor-eine-änderung-fertig-ist) besteht, `pnpm release:check` eingeschlossen, CI auf `dev` ist grün, und `CHANGELOG.md` nennt alles, was das Release ändert.
2. **Die Modelldaten aktualisieren**, damit eine frische Installation mit aktuellen Preisen und Modellen startet, und das Ergebnis auf `dev` committen:

```bash
cargo xtask models-dev
```

3. **`dev` auf `prod` pushen.** Der Push ist ein Fast-Forward. Wird er abgelehnt, hat `dev` das letzte Release noch nicht: zuerst `prod` in `dev` mergen, wie in Schritt 5.

```bash
git switch dev
git pull --ff-only
git push origin dev:prod
```

4. **Den Release-Workflow auf `prod` ausführen**, mit der Version, etwa `0.2.0` oder `0.2.0-rc.1`; ein vorangestelltes `v` fällt weg. In den **Actions** des Repositorys ist das **Release**, dann **Run workflow** mit dem Branch `prod`, oder im Terminal:

```bash
gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.0
```

5. **Prüfen, dass `dev` das Release hat.** Der Job **Sync dev** des Laufs bringt es dorthin. Schlägt dieser Job fehl, ist das Release selbst trotzdem fertig, und seine Zusammenfassung nennt den Grund; dann `prod` von Hand in `dev` mergen, Konflikte auflösen, den Merge mit `git commit --no-edit` abschließen und `dev` pushen:

```bash
git fetch origin
git switch dev
git pull --ff-only
git merge --no-ff -m "Merge release 0.2.0 into dev" origin/prod
git push origin dev
```

6. **Die Dokumentation veröffentlichen**, mit dem Pages-Workflow und dem Tag des Releases, etwa `v0.2.0`. Nach einem Pre-Release ist das optional, denn die Website beschreibt dann das Pre-Release.

```bash
gh workflow run pages.yml --repo logicplanes/kaeso --ref v0.2.0
```

## Pre-Releases

Ein Pre-Release wie `0.2.0-rc.1` entsteht aus `prod` wie jedes Release. Es erhält auf npm den Tag `next`, ist auf GitHub als Pre-Release markiert und erhält als Image-Tag nur seine Version, sodass `latest` beim neuesten stabilen Release bleibt. `prod` hält danach das Pre-Release, und das nächste Release ist ein weiteres Pre-Release oder die stabile Version. Jedes Pre-Release erhält einen eigenen Abschnitt im Changelog, bis das stabile Release sie zu einem zusammenfasst ([das Changelog](#das-changelog)); das stabile Release braucht deshalb keine eigenen neuen Zeilen.

## Hotfixes

Ein Hotfix korrigiert das neueste Release ohne das, was sich seitdem auf `dev` gesammelt hat. Das geht, solange `prod` noch auf diesem Release steht.

1. **Vom Tag des Releases abzweigen**, etwa von `v0.2.0` für den Hotfix `0.2.1`:

```bash
git fetch origin --tags
git switch -c hotfix/0.2.1 v0.2.0
```

2. **Den Fehler beheben**, mit einem Test, der ihn zeigt, eine Zeile unter `[Unreleased]` in `CHANGELOG.md` ergänzen, committen und die Prüfungen ausführen.
3. **Den Branch auf `prod` pushen**, ein Fast-Forward, weil `prod` auf dem Commit des Releases steht:

```bash
git push origin hotfix/0.2.1:prod
```

4. **Den Release-Workflow auf `prod` ausführen**, mit der Version des Hotfixes:

```bash
gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.1
```

**Sync dev** mergt den Hotfix in `dev`. Hat `dev` Zeilen unter `[Unreleased]`, gerät `CHANGELOG.md` in Konflikt: dann von Hand mergen wie unter [ein Release erstellen](#ein-release-erstellen) und dabei die Zeilen von `dev` unter `[Unreleased]` und darunter den Abschnitt des Hotfixes behalten.

## Der Release-Workflow

`.github/workflows/release.yml` führt Validate, Prepare, Checks und Tag nacheinander aus; Sync dev und die Veröffentlichungsjobs starten, sobald der Tag gepusht ist. Die Jobs, die ins Repository schreiben oder etwas veröffentlichen, laufen in der Umgebung `release`, die nur Läufe auf `prod` annimmt. Tag und Sync dev sind die einzigen Jobs, die pushen können: Sie bauen nichts, führen nur git aus und pushen mit dem eigenen Token des Laufs, deshalb starten ihre Pushes die CI nicht erneut.

| Job | Was er tut |
| --- | --- |
| Validate | Lehnt einen Lauf auf einem anderen Branch als `prod` ab, eine Version, die nicht semantisch ist, einen Tag, den es schon gibt, eine Version, die nicht neuer als der letzte Tag ist, und ein `CHANGELOG.md` ohne Einträge unter `[Unreleased]` und ohne Abschnitt für die Version oder, bei einer stabilen Version, für ein Pre-Release davon |
| Prepare | Setzt die Version in jedem Crate mit `cargo set-version`, in `Cargo.lock` und `fuzz/Cargo.lock` sowie in jeder `package.json`; schreibt `openapi.json` neu; legt beide Images in `deploy/compose.yaml` fest; verschiebt das Changelog und fasst dabei die Abschnitte der Pre-Releases einer stabilen Version in deren eigenem zusammen. Er kann das Repository nur lesen, hat keine Git-Zugangsdaten und gibt diese Änderungen als Patch an Tag weiter |
| Checks | Führt den ganzen CI-Workflow aus, jeden Job, egal was sich geändert hat |
| Tag | Wendet den Patch an und lehnt einen ab, der eine Datei anlegt, löscht oder umbenennt oder eine andere Datei ändert als die, die Prepare schreibt. Ändert der Patch eine Datei, committet er „Release X.Y.Z“ auf `prod`; ändert er keine, wie bei `0.1.0`, kommt der Tag auf den Commit von `prod` selbst. Er bricht ab, wenn sich `prod` seit dem Start des Laufs bewegt hat, taggt `vX.Y.Z` und pusht `prod` und den Tag gemeinsam, beides oder keins |
| Sync dev | Bringt das Release nach `dev`: als Fast-Forward, wenn `dev` nichts Neues hat, sonst als Merge-Commit „Merge release X.Y.Z into dev“, wenn sich das sauber mergen lässt. Bei einem Konflikt, wenn das Release eine Workflow-Datei ändert, die `dev` fehlt, oder wenn sich `dev` inzwischen bewegt, schlägt er fehl, und seine Zusammenfassung sagt, wie man von Hand mergt |
| Image | Baut jedes Image für jede Plattform aus dem Tag, mit vollständiger Build-Provenienz und einer SBOM |
| Image manifest | Fasst beide Plattformen unter den Image-Tags oben zusammen, überträgt die `org.opencontainers.image`-Labels der Images als Annotationen auf den gemeinsamen Index und bestätigt die Provenienz des Builds |
| Pack npm packages | Baut die Bibliotheken und führt die Paketprüfung mit der Version des Releases aus |
| npm | Veröffentlicht `@logicplanes/kaeso-tokens` und `@logicplanes/kaeso-ui` |
| crates.io | Veröffentlicht `kaeso-kpp` und `kaeso-codecs` |
| GitHub release | Lädt die veröffentlichten Tarballs und Crates herunter und erstellt das Release mit seinen Dateien und Release Notes |

Die Jobs für npm und crates.io melden sich über Trusted Publishing an, sodass das Repository keine Registry-Tokens enthält. Sie überspringen eine Version, die schon veröffentlicht ist; schlägt ein Veröffentlichungsjob fehl, schließt das erneute Ausführen der fehlgeschlagenen Jobs des Laufs das Release ab.

## Der Pages-Workflow

`.github/workflows/pages.yml` läuft auf dem Tag des Releases, das veröffentlicht wird, gewählt als Ref des Workflows, und verweigert jeden Lauf, der nicht von einem Release-Tag kommt. Er installiert den Workspace, baut die Bibliotheken mit `pnpm build:packages`, baut diese Website mit `pnpm --filter kaeso-docs build`, Budgets eingeschlossen, und stellt `apps/docs/dist` auf GitHub Pages unter docs.kaeso.app bereit. Die Dokumentation beschreibt das Release, aus dem sie veröffentlicht wurde.
