---
title: Releases
description: Wie Versionen nummeriert werden, was ein Release veröffentlicht und wie der Maintainer eines erstellt.
order: 9
---

Kaeso wird als Ganzes veröffentlicht: Das Gateway, die Konsole, die Crates von KPP und die Pakete der UI-Bibliothek teilen sich eine Versionsnummer, und ein Release veröffentlicht sie alle. Mitwirkende erstellen nie Tags oder Releases; der Maintainer erstellt jedes Release mit dem Release-Workflow des Repositorys und veröffentlicht die Dokumentation mit dessen Pages-Workflow.

## Versionen

Versionen folgen [Semantic Versioning](https://semver.org). Ein Release, das von Betreibern von Kaeso einen manuellen Schritt verlangt, etwa eine Migration, die nicht von selbst laufen kann, ist ein Major-Release. Ein Pre-Release hängt ein Suffix an, etwa `0.2.0-rc.1`. Der Git-Tag eines Releases ist `v` gefolgt von seiner Version, etwa `v0.1.0`.

Gateway und Konsole eines Releases gehören immer zusammen: Die `compose.yaml` des Releases legt beide Images auf diese Version fest, und die Konsole erwartet genau die Platform API ihres Gateways.

## Branches

| Branch | Was er ist |
| --- | --- |
| `dev` | Der Standard-Branch. Jeder Pull Request geht hierhin, und Releases entstehen aus ihm |
| `prod` | Das neueste stabile Release. Nur der Release-Workflow bewegt ihn, auf den Commit eines Releases, das kein Pre-Release ist |
| `hotfix/*` | Eine Korrektur für das neueste Release, von `prod` abgezweigt und aus dem Branch selbst veröffentlicht |

Nach einem Hotfix-Release hat `prod` einen Commit, den `dev` nicht hat; `prod` in `dev` mergen, sonst wird das nächste Release aus `dev` abgelehnt.

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

Ein Release fügt unter `## [Unreleased]` die Überschrift `## [X.Y.Z] - YYYY-MM-DD` ein, sodass die dort gesammelten Zeilen zum Abschnitt der Version werden. Am Ende der Datei vergleicht `[Unreleased]` danach mit dem neuen Tag, und ein neuer Link zeigt auf das Release der Version. Dieser Abschnitt wird zu den Release Notes. Ein Release ohne Einträge unter `[Unreleased]` wird abgelehnt, es sei denn, `CHANGELOG.md` hat schon einen Abschnitt für diese Version.

## Ein Release erstellen

1. **`dev` vorbereiten.** Jede Prüfung der [Definition of Done](testing.md#bevor-eine-änderung-fertig-ist) besteht, `pnpm release:check` eingeschlossen, und `CHANGELOG.md` nennt alles, was das Release ändert.
2. **Die Modelldaten aktualisieren**, damit eine frische Installation mit aktuellen Preisen und Modellen startet, und das Ergebnis committen:

```bash
cargo xtask models-dev
```

3. **Den Release-Workflow ausführen.** In den **Actions** des Repositorys **Release** wählen, dann **Run workflow**. Den Branch wählen, `dev` oder einen `hotfix/*`-Branch, und die Version eingeben, etwa `0.2.0` oder `0.2.0-rc.1`; ein vorangestelltes `v` fällt weg.
4. **Die Dokumentation veröffentlichen.** **Pages** wählen, dann **Run workflow**, und den Tag des Releases eingeben, etwa `v0.2.0`. Der Standardwert `prod` ist das neueste stabile Release; nach einem Pre-Release dessen Tag eingeben, um seine Dokumentation zu veröffentlichen.

## Der Release-Workflow

`.github/workflows/release.yml` führt diese Jobs der Reihe nach aus. Die Jobs, die ins Repository schreiben oder etwas veröffentlichen, laufen in der Umgebung `release`.

| Job | Was er tut |
| --- | --- |
| Validate | Lehnt einen anderen Branch als `dev` oder `hotfix/*` ab, eine Version, die nicht semantisch ist, einen Tag, den es schon gibt, eine Version, die nicht neuer als der letzte Tag ist, und ein leeres `[Unreleased]` |
| Checks | Führt den ganzen CI-Workflow aus, jeden Job, egal was sich geändert hat |
| Tag | Setzt die Version in jedem Crate mit `cargo set-version`, in `Cargo.lock` und `fuzz/Cargo.lock` sowie in jeder `package.json`; schreibt `openapi.json` neu; legt beide Images in `deploy/compose.yaml` fest; verschiebt das Changelog. Er committet „Release X.Y.Z“ auf den Branch, bricht ab, wenn `prod` Commits hat, die dem Branch fehlen, taggt `vX.Y.Z` und pusht Branch und Tag gemeinsam, dazu `prod`, sofern die Version kein Pre-Release ist |
| Image | Baut jedes Image für jede Plattform aus dem Tag, mit vollständiger Build-Provenienz und einer SBOM |
| Image manifest | Fasst beide Plattformen unter den Image-Tags oben zusammen und bestätigt die Provenienz des Builds |
| Pack npm packages | Baut die Bibliotheken und führt die Paketprüfung mit der Version des Releases aus |
| npm | Veröffentlicht `@logicplanes/kaeso-tokens` und `@logicplanes/kaeso-ui` |
| crates.io | Veröffentlicht `kaeso-kpp` und `kaeso-codecs` |
| GitHub release | Lädt die veröffentlichten Tarballs und Crates herunter und erstellt das Release mit seinen Dateien und Release Notes |

Die Jobs für npm und crates.io überspringen eine Version, die schon veröffentlicht ist; schlägt ein Veröffentlichungsjob fehl, schließt das erneute Ausführen der fehlgeschlagenen Jobs des Laufs das Release ab.

## Der Pages-Workflow

`.github/workflows/pages.yml` hat eine Eingabe, den Tag oder Branch, der veröffentlicht wird. Er installiert den Workspace, baut die Bibliotheken mit `pnpm build:packages`, baut diese Website mit `pnpm --filter kaeso-docs build`, Budgets eingeschlossen, und stellt `apps/docs/dist` auf GitHub Pages unter docs.kaeso.app bereit. Die Dokumentation beschreibt das Release, aus dem sie veröffentlicht wurde.
