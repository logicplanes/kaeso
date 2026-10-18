---
title: Releases
description: Wie Versionen und das Changelog funktionieren und wie der Maintainer ein Release erstellt, veröffentlicht und verteilt.
order: 11
---

Kaeso erscheint als Ganzes. Gateway, Konsole, die Crates von KPP und die Pakete der UI-Bibliothek teilen sich eine Version, und ein Lauf des Release-Workflows veröffentlicht sie alle. Nur der Maintainer erstellt Releases; Mitwirkende legen nie Tags oder Releases an.

## Versionen

Versionen folgen [Semantic Versioning](https://semver.org). Ein Release, das von Betreibern einen Schritt von Hand verlangt, etwa eine Migration, die nicht von selbst laufen kann, ist ein Major-Release. Eine Vorabversion hängt ein Suffix an, etwa `0.2.0-rc.1`. Der Git-Tag ist `v` plus Version, etwa `v0.2.0`.

Gateway und Konsole eines Releases gehören zusammen. Die `compose.yaml` des Releases legt beide Images auf diese Version fest, und die Konsole erwartet genau die Platform API ihres Gateways.

## Die Version im Repository

Die `package.json` im Wurzelverzeichnis enthält die Version, und nur ein Release ändert sie, mit `node scripts/version.mjs set <version>`. Dieser Befehl schreibt die Version in jede Datei, die sie nennen muss, etwa die übrigen `package.json`-Dateien, die Manifeste der Crates, `openapi.json` und `deploy/compose.yaml`. `node scripts/version.mjs files` listet sie auf.

Alles andere liest die Version, statt sie zu nennen. Eine Seite schreibt `\{{version}}`, das Gateway liest `env!("CARGO_PKG_VERSION")`, und erzeugte Dateien lesen ihre `package.json`. `node scripts/version.mjs check`, Teil von `pnpm test`, schlägt fehl, wenn die aktuelle Version irgendwo anders steht, und nennt Datei und Zeile.

## Branches

| Branch | Was er ist | Wer ihn ändert |
| --- | --- | --- |
| `dev` | Der Standard-Branch, auf dem alle Arbeit landet | Nur Pull Requests ([Pull Request mergen lassen](contributing.md#pull-request-mergen-lassen)) |
| `prod` | Das neueste Release oder der Commit, der gleich eines wird | Der Maintainer, immer als Fast-Forward |
| `hotfix/X.Y.Z` | Eine Korrektur für das neueste Release, von dessen Tag abgezweigt | Der Maintainer, der ihn nach `prod` pusht |

`dev` und `prod` lassen sich weder löschen noch per Force-Push überschreiben, und ein `v*`-Tag lässt sich weder verschieben noch löschen.

## Das Changelog

`CHANGELOG.md` folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), und Pull Requests ändern die Datei nie. Der Release-Workflow schreibt den Abschnitt jeder Version aus den Pull Requests, die seit dem letzten Release gemergt wurden. Der Titel jedes Pull Requests wird zu seiner Zeile, mit einem Link auf den Pull Request, und seine Labels bestimmen, wo die Zeile steht.

Pull Requests mit dem Label `internal` werden ausgelassen. Die Updates von Dependabot werden zu einer Zeile, „Dependencies updated“. Ein Pull Request mit dem Label `breaking` wird zusätzlich oben im Abschnitt genannt.

Um die Notizen selbst zu schreiben, setzen Sie Zeilen unter `[Unreleased]` in die `CHANGELOG.md` auf `dev`. Das Release nutzt dann nur diese Zeilen. Das eignet sich für ein Release, dessen Notizen mehr als Titel brauchen, und für einen Hotfix, dessen Commits keinen Pull Request haben.

Hat das Repository das Secret `LOG_AI_KEY`, lässt der Workflow den Abschnitt von einem Modell über die Kaeso-Installation des Maintainers entwerfen, aus den gemergten Pull Requests. Ihre Zeilen unter `[Unreleased]` übernimmt es wörtlich. Besteht der Entwurf eine Prüfung nicht oder schlägt die Anfrage fehl, greift das Release auf die Titel zurück. Um die Notizen des nächsten Releases zu sehen, ohne eine Datei zu ändern:

```bash
pnpm release:notes --dry-run --since v{{version}} --head origin/dev
```

Ein stabiles Release nach Vorabversionen seiner Version fasst deren Abschnitte zu einem zusammen, sodass seine Notizen alles seit dem letzten stabilen Release abdecken.

### Labels

| Label | Im Changelog |
| --- | --- |
| `added`, `changed`, `deprecated`, `removed`, `fixed`, `security` | Der Abschnitt, etwa Added. Ohne Label entscheidet das erste Wort des Titels: Add, Fix, Remove und so weiter, sonst Changed |
| `breaking` | Zusätzlich oben genannt, als Breaking Change |
| `internal` | Ausgelassen |
| `area: gateway`, `area: kpp`, `area: console`, `area: ui library`, `area: documentation`, `area: self-hosting` | Das Thema innerhalb des Abschnitts. Genau ein Bereichs-Label legt es fest |

Der Workflow **Labels** setzt die Bereichs-Labels und `internal` bei jedem Push anhand der geänderten Dateien, nach `.github/labeler.yml`. Ein von Hand gesetztes Label hält bis zum nächsten Push. Der Maintainer setzt das Typ-Label, wenn das erste Wort des Titels nicht den richtigen Abschnitt wählt, und `breaking`, wenn Betreiber für ein Upgrade etwas ändern müssen.

## Release erstellen

1. Prüfen Sie, dass `dev` bereit ist: CI ist grün, jeder gemergte Pull Request hat die Labels, die seine Zeile braucht, und die Prüfungen unter [Bevor eine Änderung fertig ist](testing.md#bevor-eine-änderung-fertig-ist) bestehen.
2. Aktualisieren Sie die Modelldaten, damit eine frische Installation mit aktuellen Modellen und Preisen startet, und mergen Sie das Ergebnis über einen Pull Request nach `dev`:

   ```bash
   cargo xtask models-dev
   ```

3. Pushen Sie `dev` als Fast-Forward nach `prod`:

   ```bash
   git switch dev
   git pull --ff-only
   git push origin dev:prod
   ```

4. Starten Sie den Release-Workflow auf `prod` mit der Version. Setzen Sie in den Eingaben des Workflows **manual**, wenn das Release Schritte von Hand braucht, damit Installationen es nie mit einem Klick anwenden:

   ```bash
   gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.0
   ```

5. Bringen Sie das Release nach `dev`. Der Job **Sync dev** des Laufs versucht es, aber `dev` nimmt nur Pull Requests an. Er endet deshalb meist mit einer Warnung und zeigt den Befehl, den Sie ausführen. Mergen Sie von Hand, was Ihre Ausnahme von den Regeln für `dev` erlaubt:

   ```bash
   git fetch origin
   git switch dev
   git pull --ff-only
   git merge --no-ff -m "Merge release 0.2.0 into dev" origin/prod
   git push origin dev
   ```

6. Verteilen Sie die Websites vom Tag des Releases ([Websites verteilen](#websites-verteilen)).

Wird der Push in Schritt 3 abgelehnt, hat `dev` das letzte Release noch nicht: Mergen Sie zuerst `prod` nach `dev`, wie in Schritt 5. Schlägt ein Veröffentlichungs-Job fehl, starten Sie die fehlgeschlagenen Jobs des Laufs neu. Jeder überspringt, was schon veröffentlicht ist.

Ein Release veröffentlicht:

| Wo | Was |
| --- | --- |
| GitHub-Releases | `vX.Y.Z` mit seinen Notizen, `compose.yaml`, `kaeso.example.toml`, `env.example`, `release.json`, den npm-Tarballs und den `.crate`-Dateien |
| GitHub Container Registry | `kaeso-gateway`, `kaeso-console` und `kaeso-updater` für `linux/amd64` und `linux/arm64`, mit Build-Provenienz und einer Software-Stückliste (SBOM) |
| crates.io | `kaeso-kpp` und `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` und `@logicplanes/kaeso-tokens` |

Images bekommen die Version als Tag und bei stabilen Releases außerdem `MAJOR.MINOR`, `latest` und ab 1.0.0 `MAJOR`.

## Vorabversion erstellen

Starten Sie den Release-Workflow auf `prod` mit einer Version wie `0.2.0-rc.1`. Eine Vorabversion bekommt auf npm den Tag `next` und als Image-Tag nur ihre Version, und GitHub markiert sie nie als neuestes Release. Die Websites danach zu verteilen ist optional.

## Hotfix erstellen

Ein Hotfix korrigiert das neueste Release ohne das, was sich seitdem auf `dev` gesammelt hat. Er funktioniert, solange `prod` noch auf diesem Release steht.

1. Zweigen Sie vom Tag des Releases ab:

   ```bash
   git fetch origin --tags
   git switch -c hotfix/0.2.1 v0.2.0
   ```

2. Beheben Sie den Fehler mit einem Test, der ihn zeigt, und setzen Sie eine Zeile unter `[Unreleased]` in die `CHANGELOG.md`. Diese Zeile ist das Changelog des Hotfixes.
3. Pushen Sie den Branch nach `prod`:

   ```bash
   git push origin hotfix/0.2.1:prod
   ```

4. Starten Sie den Release-Workflow auf `prod` mit der Version des Hotfixes.
5. Bringen Sie den Hotfix nach `dev`, wie unter [Release erstellen](#release-erstellen). Gibt es einen Konflikt in `CHANGELOG.md`, behalten Sie die Zeilen von `dev` unter `[Unreleased]` und den Abschnitt des Hotfixes darunter.

## Websites verteilen

Die Dokumentation unter docs.kaeso.app und der Showcase unter ui.kaeso.app sind statische Websites, gebaut vom Tag eines Releases:

```bash
git fetch origin --tags
git checkout v0.2.0
pnpm install --frozen-lockfile && pnpm build:packages
pnpm --filter kaeso-docs build
pnpm --filter kaeso-showcase build
```

Laden Sie `apps/docs/dist` und `apps/showcase/dist` unverändert hoch. Sie enthalten jede Seite in jeder Sprache, `robots.txt`, `sitemap.xml` und eine `404.html` pro Sprache. Der Host muss `/console` mit `console.html` beantworten und eine unbekannte Adresse mit der `404.html` ihrer Sprache. Verschobene Seiten haben eine Seite unter ihrer alten Adresse, und `_redirects` listet dieselben Umzüge für Hosts, die diese Datei lesen.

### Content Security Policy

Jeder Build schreibt die Richtlinie seiner Website als `<meta>`-Tag in jede Seite sowie in `csp.txt` und `_headers`. Senden Sie sie außerdem als Header `Content-Security-Policy`, denn nur der Header kann `frame-ancestors 'none'` hinzufügen. Der Header der Dokumentation ändert sich nie:

```text
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'
```

Die Richtlinie des Showcases nennt einen Hash für jedes seiner Inline-Skripte und -Stile; die Hashes ändern sich mit ihnen. Nehmen Sie seinen Header aus der `csp.txt` des Builds, den Sie verteilen.
