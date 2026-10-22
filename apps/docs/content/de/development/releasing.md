---
title: Releases
description: Wie Versionen und das Changelog funktionieren und wie der Maintainer ein Release erstellt, veröffentlicht und verteilt.
order: 11
---

Kaeso erscheint als Ganzes. Gateway, Konsole, die Crates von KPP und die Pakete der UI-Bibliothek teilen sich eine Version, und ein Lauf des Release-Workflows veröffentlicht sie alle. Nur der Maintainer erstellt Releases; Mitwirkende legen nie Tags oder Releases an.

## Versionen

Versionen folgen [Semantic Versioning](https://semver.org). Ein Release, das von Betreibern einen Schritt von Hand verlangt, etwa eine Migration, die nicht von selbst laufen kann, ist ein Major-Release. Eine Vorabversion hängt ein Suffix an, etwa `0.2.0-rc.1`. Der Git-Tag ist `v` plus Version, etwa `v0.2.0`.

Gateway und Konsole eines Releases gehören zusammen. Die `compose.yaml` des Releases legt beide Images auf diese Version und die Digests fest, die das Release gebaut hat, und die Konsole erwartet genau die Platform API ihres Gateways.

## Die Version im Repository

Die `package.json` im Wurzelverzeichnis enthält die Version, und nur ein Release ändert sie, mit `node scripts/version.mjs set <version>`. Dieser Befehl schreibt die Version in jede Datei, die sie nennen muss, etwa die übrigen `package.json`-Dateien, die Manifeste der Crates, `openapi.json` und `deploy/compose.yaml`. `node scripts/version.mjs files` listet sie auf.

Alles andere liest die Version, statt sie zu nennen. Eine Seite schreibt `\{{version}}`, das Gateway liest `env!("CARGO_PKG_VERSION")`, und erzeugte Dateien lesen ihre `package.json`. `node scripts/version.mjs check`, Teil von `pnpm test`, schlägt fehl, wenn die aktuelle Version irgendwo anders steht, und nennt Datei und Zeile.

## Branches

| Branch | Was er ist | Wer ihn ändert |
| --- | --- | --- |
| `dev` | Der Standard-Branch, auf dem alle Arbeit landet | Nur Pull Requests ([Pull Request mergen lassen](contributing.md#pull-request-mergen-lassen)) |
| `prod` | Der Commit, auf dem das neueste Release lief, oder der Commit, der gleich ein Release wird | Der Maintainer, immer als Fast-Forward |
| `hotfix/X.Y.Z` | Eine Korrektur für das neueste Release, von dessen Tag abgezweigt | Der Maintainer, der ihn nach `prod` pusht |

`dev` und `prod` lassen sich weder löschen noch per Force-Push überschreiben, nur der Maintainer kann nach `prod` pushen, und ein `v*`-Tag lässt sich weder verschieben noch löschen.

## Das Changelog

`CHANGELOG.md` folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), und Pull Requests ändern die Datei nie. Der Release-Workflow schreibt den Abschnitt jeder Version aus den Pull Requests, die seit dem letzten Release gemergt wurden. Der Titel jedes Pull Requests, wie er gemergt wurde, wird zu seiner Zeile, mit einem Link auf den Pull Request, und seine Labels bestimmen, wo die Zeile steht. Dieser Titel ist der Betreff des Squash-Commits des Pull Requests auf `prod`, daher ändert eine Bearbeitung nach dem Merge nichts. Für einen auf andere Weise gemergten Pull Request ist es der Titel zum Zeitpunkt des Merges.

Pull Requests mit dem Label `internal` werden ausgelassen. Die Updates von Dependabot werden zu einer Zeile, „Dependencies updated“. Ein Pull Request mit dem Label `breaking` wird zusätzlich oben im Abschnitt genannt.

Um die Notizen selbst zu schreiben, setzen Sie Zeilen unter `[Unreleased]` in die `CHANGELOG.md` auf `dev`. Das Release stellt diese Zeilen an den Anfang und ergänzt eine Zeile für jeden Pull Request, den keine von ihnen als `[#42](…/pull/42)` verlinkt. Das eignet sich für ein Release, dessen Notizen mehr als Titel brauchen, und für einen Hotfix, dessen Commits keinen Pull Request haben.

Hat die Umgebung `release` des Repositorys das Secret `LOG_AI_KEY`, lässt der Workflow den Abschnitt von einem Modell über die Kaeso-Installation des Maintainers entwerfen, aus den gemergten Pull Requests. Das Modell liest jeden so, wie er gemergt wurde: Betreff und Text des Squash-Commits oder die Commits eines auf andere Weise gemergten Pull Requests. Den Titel eines verknüpften Issues sieht es nur, wenn dessen Autor Schreibrechte im Repository hat. Ihre Zeilen unter `[Unreleased]` übernimmt es wörtlich. Besteht der Entwurf eine Prüfung nicht oder schlägt die Anfrage fehl, greift das Release auf die Titel zurück. Um die Notizen des nächsten Releases zu sehen, ohne eine Datei zu ändern:

```bash
pnpm release:notes --dry-run --since v{{version}} --head origin/dev
```

Ein stabiles Release nach Vorabversionen seiner Version fasst deren Abschnitte zu einem zusammen, sodass seine Notizen alles seit dem letzten stabilen Release abdecken.

Das GitHub-Release zeigt eine Kurzfassung des Abschnitts. Sie hat einen Satz, bis zu fünf Highlights, die Breaking Changes, die Mitwirkenden und Links zum vollständigen Changelog und zum Vergleich. Das Modell schreibt sie zusammen mit dem Abschnitt. Ohne Modell listet das Release die ersten fünf Zeilen des Abschnitts, jede auf ihren ersten Satz gekürzt. Die Vorschau oben gibt nach den Notizen auch das GitHub-Release aus.

### Was die Notizen enthalten dürfen

Jede Zeile der Notizen, vom Modell oder aus einem Titel, besteht diese Prüfungen:

- Keine Adressen: keine URLs, E-Mail-Adressen oder IP-Adressen.
- Keine Hostnamen außer `kaeso.app` und seinen Subdomains, `github.com/logicplanes/kaeso` und Kaesos Images unter `ghcr.io/logicplanes`. `crates.io` und `models.dev` dürfen ohne Pfad vorkommen. Ein Name, der auf eine gängige Top-Level-Domain wie `.com` oder `.example` endet, zählt als Hostname, auch in Code-Spans.
- Keine Image-Referenzen außerhalb von `ghcr.io/logicplanes/`, etwa `caddy:2.11.7-alpine` oder `example/kaeso-gateway@sha256:…`.
- Keine Shell-Pipelines wie `| sh` oder `curl … | jq` und keine Download-Befehle wie `curl -fsSL` oder `wget`.
- Keine Befehle, die etwas laden, installieren oder ausführen: `docker run`, `docker pull`, `npx`, `pnpm dlx`, `npm install`, `npm i` oder `pip install`.
- Keine Anweisungen, eine Umgebungsvariable zu setzen, etwa `set KAESO_…`, `export KAESO_…` oder `KAESO_…=…`. Eine Einstellung zu nennen ist erlaubt.

Ein Entwurf, der nach einer Korrektur noch scheitert, fällt auf die Titel zurück. Ein Titel, der scheitert, hält den Job **Prepare** an und nennt den Pull Request. Schreiben Sie dafür eine Zeile unter `[Unreleased]` auf `dev`, die den Pull Request verlinkt, und starten Sie Release erneut.

### Prüfung vor dem Tag

Jeder Job, der ein Secret nutzt, ins Repository schreibt oder veröffentlicht, läuft in der Umgebung `release` und wartet daher auf die Freigabe des Maintainers. Die Freigabe von **Release notes** lässt das Modell entwerfen. **Prepare** schreibt danach das Changelog und zeigt den Abschnitt und das GitHub-Release in seiner Zusammenfassung, genau so, wie das Release sie veröffentlicht. **Tag** wartet danach auf Ihre Freigabe, lesen Sie also zuerst die Zusammenfassung. Stimmt darin etwas nicht, lehnen Sie **Tag** ab, beheben Sie die Ursache auf `dev` und starten Sie Release erneut.

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

4. Starten Sie den Release-Workflow auf `prod` mit der Version. Setzen Sie in den Eingaben des Workflows **manual**, wenn das Release Schritte von Hand braucht, damit der Updater es nie installiert:

   ```bash
   gh workflow run release.yml --repo logicplanes/kaeso --ref prod -f version=0.2.0
   ```

   Geben Sie die Jobs des Laufs frei, sobald sie warten. Bevor Sie **Tag** freigeben, lesen Sie die Notizen in der Zusammenfassung von **Prepare** ([Prüfung vor dem Tag](#prüfung-vor-dem-tag)).

5. Bringen Sie das Release nach `dev`. Der Job **Sync dev** des Laufs versucht es, aber `dev` nimmt nur Pull Requests an. Er endet deshalb meist mit einer Warnung und zeigt den Befehl, den Sie ausführen. Mergen Sie von Hand, was Ihre Ausnahme von den Regeln für `dev` erlaubt:

   ```bash
   git fetch origin --tags
   git switch dev
   git pull --ff-only
   git merge --no-ff -m "Merge release 0.2.0 into dev" v0.2.0
   git push origin dev
   ```

6. Verteilen Sie die Websites vom Tag des Releases ([Websites verteilen](#websites-verteilen)).

Bricht der Lauf ab, weil `prod` das letzte Release noch nicht hat, mergen Sie dessen Tag wie in Schritt 5 nach `dev` und wiederholen Sie Schritt 3. Schlägt ein Veröffentlichungs-Job fehl, starten Sie die fehlgeschlagenen Jobs des Laufs neu. Jeder überspringt, was schon veröffentlicht ist, und der Job für das GitHub-Release ersetzt Assets und Notizen eines Releases, das er schon angelegt hat.

Ein Release veröffentlicht:

| Wo | Was |
| --- | --- |
| GitHub-Releases | `vX.Y.Z` mit seinen Notizen, `compose.yaml`, `kaeso.example.toml`, `env.example`, `release.json`, den npm-Tarballs und den `.crate`-Dateien |
| GitHub Container Registry | `kaeso-gateway`, `kaeso-console` und `kaeso-updater` für `linux/amd64` und `linux/arm64`, mit Build-Provenienz und einer Software-Stückliste (SBOM) |
| crates.io | `kaeso-kpp` und `kaeso-codecs` |
| npm | `@logicplanes/kaeso-ui` und `@logicplanes/kaeso-tokens` |

Images bekommen die Version als Tag und bei stabilen Releases außerdem `MAJOR.MINOR`, `latest` und ab 1.0.0 `MAJOR`.

### Saubere Builds

Release-Jobs stellen keinen Cache wieder her, den ein anderer Lauf geschrieben haben könnte. Die Images bauen mit `no-cache`, pnpm und Cargo beginnen mit einem leeren Speicher. Die npm-Pakete werden aus einer Installation ohne Lifecycle-Skripte gepackt. Der Job **crates.io build** packt und kompiliert `kaeso-kpp` und `kaeso-codecs` ohne Veröffentlichungstoken. Erst der Job **crates.io** hält das Token und lädt sie mit `cargo publish --no-verify` hoch, sodass er kein Build-Skript ausführt.

### Commits und Attestierungen

Der Lauf beginnt auf dem Commit von `prod`. Ändert sich die Version, committet der Job Tag darauf `Release X.Y.Z` und taggt diesen Commit als `vX.Y.Z`. Der Release-Commit ändert nur `CHANGELOG.md` und die Dateien, die `node scripts/version.mjs files` auflistet. Vor dem Commit prüft der Job Tag, dass diese Änderungen außer `CHANGELOG.md` dem entsprechen, was `node scripts/version.mjs set` in einem eigenen Job schreibt.

Der Job Tag pusht nur den Tag: `prod` bekommt den Release-Commit mit dem nächsten Push von `dev`, das ihn aus Schritt 5 hat. Jedes Image, jedes Paket und jedes Asset wird aus dem Tag gebaut.

`gh attestation verify` und der Updater prüfen die Attestierungen der Images und von `release.json`. Sie nennen den Commit, auf dem der Lauf begann, denn GitHub signiert sie für den Lauf, und `release.json` nennt denselben Commit. Die Build-Provenienz in jedem Image, die BuildKit schreibt, nennt den Commit des Tags als Quelle. Der Commit des Tags ist also der attestierte Commit oder ein Release-Commit direkt darauf, der nur Version und Changelog ändert.

## Vorabversion erstellen

Starten Sie den Release-Workflow auf `prod` mit einer Version wie `0.2.0-rc.1`. Eine Vorabversion bekommt auf npm den Tag `next` und als Image-Tag nur ihre Version, und GitHub markiert sie nie als neuestes Release. Die Websites danach zu verteilen ist optional.

## Hotfix erstellen

Ein Hotfix korrigiert das neueste Release ohne das, was sich seitdem auf `dev` gesammelt hat. Er funktioniert, solange `prod` noch auf dem Commit steht, auf dem dieses Release lief.

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

Jeder Build schreibt die Richtlinie seiner Website als `<meta>`-Tag in jede Seite sowie in `csp.txt` und `_headers`. Senden Sie sie außerdem als Header `Content-Security-Policy`, denn nur der Header kann `frame-ancestors 'none'` hinzufügen. Der Header der Dokumentation ändert sich nie; er erlaubt Bilder von `update.kaeso.app`, woher die Release-Bilder im Änderungsprotokoll kommen:

```text
default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: https://update.kaeso.app; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'
```

Die Richtlinie des Showcases nennt einen Hash für jedes seiner Inline-Skripte und -Stile; die Hashes ändern sich mit ihnen. Nehmen Sie seinen Header aus der `csp.txt` des Builds, den Sie verteilen.
