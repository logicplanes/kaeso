---
title: Releases
description: Wie Versionen nummeriert werden, was ein Release veröffentlicht und wie der Maintainer eines erstellt.
order: 9
---

Kaeso wird als Ganzes veröffentlicht: Das Gateway, die Konsole, die Crates von KPP und die Pakete der UI-Bibliothek teilen sich eine Versionsnummer, und ein Release veröffentlicht sie alle. Mitwirkende erstellen nie Tags oder Releases; der Maintainer erstellt jedes Release mit dem Release-Workflow des Repositorys.

## Versionen

Versionen folgen [Semantic Versioning](https://semver.org). Ein Release, das von Betreibern von Kaeso einen manuellen Schritt verlangt, etwa eine Migration, die nicht von selbst laufen kann, ist ein Major-Release. Der Git-Tag eines Releases ist `v` gefolgt von seiner Version, etwa `v0.1.0`.

Gateway und Konsole eines Releases gehören immer zusammen: Die `compose.yaml` des Releases legt beide Images auf diese Version fest, und die Konsole erwartet genau die Platform API ihres Gateways.

## Was ein Release veröffentlicht

| Wo | Was |
| --- | --- |
| GitHub Releases | Das Release `vX.Y.Z` mit seinen Release Notes aus `CHANGELOG.md` und der angehängten Datei `compose.yaml`, die [Installationen](../self-hosting/installation.md) herunterladen |
| GitHub Container Registry | `ghcr.io/logicplanes/kaeso-gateway` und `ghcr.io/logicplanes/kaeso-console`, mit der Version getaggt |
| crates.io | `kaeso-kpp` und `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` und `@logicplanes/kaeso-tokens`, mit pnpm veröffentlicht |

Die Dokumentation unter docs.kaeso.app und der Showcase unter ui.kaeso.app werden aus demselben Repository veröffentlicht und beschreiben das neueste Release.

## Das Changelog

`CHANGELOG.md` im Wurzelverzeichnis des Repositorys folgt [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Jede Änderung, die Personen bemerken werden, ergänzt im selben Pull Request eine Zeile unter `[Unreleased]`: was sich geändert hat, aus Sicht von jemandem, der Kaeso nutzt, betreibt oder darauf aufbaut. Ein Release macht aus `[Unreleased]` die Version mit ihrem Datum, und dieser Abschnitt wird zu den Release Notes. Ein Release ohne Einträge unter `[Unreleased]` wird abgelehnt.

## Ein Release erstellen

1. **`dev` vorbereiten.** Jede Prüfung der [Definition of Done](testing.md#bevor-eine-änderung-fertig-ist) besteht, und `CHANGELOG.md` nennt alles, was das Release ändert.
2. **Die Modelldaten aktualisieren**, damit eine frische Installation mit aktuellen Preisen und Modellen startet, und das Ergebnis committen:

```bash
cargo xtask models-dev
```

3. **Die Pakete prüfen.** Nach `pnpm build` packt `pnpm release:check` die npm-Pakete und prüft, was sie enthalten, etwa die Lizenzdateien der Schriften.
4. **Den Release-Workflow ausführen.** In den **Actions** des Repositorys **Release** wählen, dann **Run workflow**, und die Version eingeben, etwa `0.2.0`. Sie muss neuer sein als das letzte Release.

Der Workflow führt die Prüfungen erneut aus, setzt die Version, taggt das Release und veröffentlicht alles aus der Tabelle oben. Ist er fertig, ist die `compose.yaml` des Releases das, was [Upgrades](../self-hosting/upgrades.md) herunterladen.
