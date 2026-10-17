---
title: Upgrades
description: Von neuen Releases erfahren, von Hand oder mit dem Updater per Klick aktualisieren, und was während des Updates geschieht.
order: 6
---

Releases folgen der semantischen Versionierung, und alle Teile von Kaeso tragen die Versionsnummer des Releases. Die Release Notes jedes Releases stehen auf der [Release-Seite](https://github.com/logicplanes/kaeso/releases) und für Instanz-Admins in der Konsole unter **Instanz-Einstellungen → Updates**. Die `compose.yaml` jedes Releases legt die Gateway-, Konsolen- und Updater-Images dieses Releases fest.

Es gibt zwei Wege zu aktualisieren: [von Hand](#von-hand-aktualisieren), was immer funktioniert, und [mit einem Klick](#updates-mit-einem-klick) in der Konsole, sobald der optionale Updater eingerichtet ist.

## Neue Releases

Das Gateway sucht zweimal täglich, zu einer zufälligen Minute innerhalb der Stunde, über die Releases-API von GitHub nach neuen Releases. Es fragt mit dem `ETag` der letzten Antwort, sodass eine unveränderte Liste GitHub nichts kostet; nach einem Fehler wartet es mindestens eine Stunde, länger, wenn GitHub darum bittet, und es versucht es nie sofort erneut. Die Anfrage enthält nichts über die Installation außer der Version des Gateways im `User-Agent`.

Unter **Instanz-Einstellungen → Updates** sehen Instanz-Admins die laufende Version, das neueste Release mit den Notizen aller Releases seither, wann das Gateway zuletzt gesucht hat und, wenn es fehlschlug, warum. **Jetzt prüfen** sucht sofort, auch wenn die Prüfungen aus sind. Eine Markierung **Neu** neben **Updates** in der Seitenleiste der Einstellungen ist der einzige Hinweis; Kaeso schickt keine E-Mails und zeigt keine Banner zu Releases.

| Einstellung | Standard | Was sie bewirkt |
| --- | --- | --- |
| Nach neuen Releases suchen (`updates.check`) | an | Für Installationen ohne Internetzugang ausschalten. |
| Releases (`updates.channel`) | nur stabile | `prerelease` bietet auch Release Candidates an. Auf dem stabilen Kanal wird nie eine Vorabversion angeboten, aber eine Installation, die eine Vorabversion ausführt, bekommt das darauf folgende stabile Release angeboten. |
| Updates mit einem Klick (`updates.apply`) | aus | Ob Instanz-Admins Updates über den Updater aus der Konsole einspielen dürfen. |

Bei einer neuen Installation kommen die drei Einstellungen aus `KAESO_UPDATES_CHECK`, `KAESO_UPDATES_CHANNEL` und `KAESO_UPDATES_APPLY`; danach ändern Instanz-Admins sie in der Konsole, und jede Änderung steht im Audit-Log der Instanz. Updates mit einem Klick einzuschalten verlangt eine Anmeldung in den letzten 15 Minuten.

### Installationen ohne Internetzugang

**Nach neuen Releases suchen** ausschalten oder vor dem ersten Start `KAESO_UPDATES_CHECK=false` in [`compose.override.yaml`](installation.md#lokale-änderungen) setzen. Die Seite zeigt dann die laufende Version, und aktualisiert wird von Hand wie unten. `KAESO_UPDATES_RELEASES_URL` verweist das Gateway auf einen Spiegel, der wie die Releases-API von GitHub antwortet, falls es einen gibt.

## Welche Releases mit einem Klick aktualisieren

Ein Release lässt sich nur mit einem Klick einspielen, wenn alles davon zutrifft; sonst zeigt die Konsole seine Notizen und einen Link auf diese Seite:

- Es ist keine neue Hauptversion. Eine Migration, die einen Schritt von Hand braucht, macht ein Release zur Hauptversion.
- Kein Release auf dem Weg verlangt Schritte von Hand. Das tut ein Release, wenn sich seine `compose.yaml` über die Image-Versionen hinaus ändert oder wenn die Maintainer es so kennzeichnen.
- Das Release trägt `release.json`, das Manifest, das der Updater liest: jedes Release ab dem ersten mit Updater.

## Von Hand aktualisieren

1. Die Release Notes auf alles prüfen, was ein Eingreifen erfordert.
2. PostgreSQL und auch ClickHouse [sichern](backups.md), damit ein [Downgrade](#downgrade) möglich bleibt.
3. Die `compose.yaml` des neuen Releases über die alte herunterladen. Die eigenen Einstellungen in `.env`, `master-keys` und `compose.override.yaml` bleiben erhalten.

```bash
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

4. Die neue Version starten:

```bash
docker compose pull
docker compose up -d --wait
```

`--wait` kehrt zurück, sobald das neue Gateway und die neue Konsole healthy melden, und schlägt fehl, wenn einer der beiden das nicht tut, sodass ein Skript oder eine Person, die das Upgrade ausführt, einen gescheiterten Start sofort sieht.

Das Gateway migriert PostgreSQL und ClickHouse beim Start unter einem Lock, sodass mehrere Instanzen gleichzeitig starten können und nur eine migriert. Migrationen laufen nur vorwärts und der Reihe nach, sodass ein Upgrade Versionen überspringen kann. Kaeso 0.1.0 legt das ganze Schema aus einem ersten Satz von Migrationen an; spätere Releases fügen ihre eigenen danach an.

Die Konsole zeigt diese Befehle mit der genauen Version, wann immer ein Klick nicht möglich ist.

## Updates mit einem Klick

Der Updater ist ein optionaler Service der `compose.yaml` im Compose-Profil `updater`, mit seinem Docker-Socket-Proxy `docker-proxy`. Er ist der einzige Teil von Kaeso, der mit Docker spricht.

### Einschalten

Dem Gateway und dem Updater ein gemeinsames Token geben und das Profil einschalten, im Ordner der Installation:

```bash
echo "KAESO_UPDATER_TOKEN=$(openssl rand -hex 32)" >> .env
echo "COMPOSE_PROFILES=updater" >> .env
docker compose up -d --wait
```

Dann unter **Instanz-Einstellungen → Updates** die **Updates mit einem Klick** einschalten. Die Seite sagt, wenn der Updater nicht installiert ist, nicht antwortet oder für ein Release zu alt ist. Kaeso-Releases vor dem Updater werden einmal von Hand aktualisiert.

Der Updater bindet `/var/run/docker.sock` über seinen Proxy ein; `DOCKER_SOCKET` in `.env` setzen, wenn der Socket von Docker woanders liegt, etwa bei Rootless Docker.

### Was geschieht

**Auf … aktualisieren** fragt nach einer Bestätigung, die die Version nennt und an die Sicherung erinnert. Das Gateway hält das Update fest, schreibt `instance.update_start` ins Audit-Log der Instanz und übergibt dem Updater die Version und sonst nichts. Die Konsole folgt dann diesen Schritten:

1. **Herunterladen.** Der Updater lädt `release.json` und `compose.yaml` über TLS aus dem Release auf github.com und lädt die Images von Gateway, Konsole und Updater über die Digests in `release.json`. Kaeso läuft weiter.
2. **Signaturen prüfen.** Für `release.json` und jedes Image lädt der Updater die [Build-Provenance-Attestierungen](https://docs.github.com/actions/security-for-github-actions/using-artifact-attestations) des Release-Workflows und prüft sie mit Sigstore: ein Zertifikat, das die Zertifizierungsstelle von Sigstore dem Release-Workflow von `logicplanes/kaeso` auf `prod` ausgestellt hat, sein Nachweis im Certificate-Transparency-Log, der signierte Eintrag im Transparenzlog, die Signatur über die Aussage, der Digest, den sie nennt, und der Commit, den `release.json` nennt. Auch das Versions-Label des Images muss passen. Die Vertrauensbasis ist in den Updater eingebaut; schlägt eine Prüfung fehl, wird nichts eingespielt.
3. **Umstellen.** Caddy antwortet mit der [Update-Seite](#während-kaeso-aktualisiert). Der Updater stoppt die Konsole, startet sie mit ihren eigenen Einstellungen und dem neuen Image wieder und wartet auf ihren Healthcheck; danach tut er dasselbe mit dem Gateway.
4. **Migrieren.** Das neue Gateway migriert beim Start die Datenbanken; der Updater wartet bis zu 10 Minuten auf seinen Healthcheck.
5. **Wieder erreichbar.** Der Updater schreibt die neuen Versionen in die `compose.yaml`, damit ein späteres `docker compose up -d --wait` dieselben Images startet, versieht sie mit der Version als Tag, entfernt die alten Container und beendet die Wartung.

Das ganze Update dauert etwa eine Minute. Es läuft immer nur ein Update. Das Ergebnis steht als `instance.update_finish` im Audit-Log der Instanz und im Verlauf der Seite: **Aktualisiert**, **Nicht eingespielt** (nichts geändert), **Zurückgesetzt** oder **Zurücksetzen fehlgeschlagen**.

### Zurücksetzen

Wird ein neuer Container nicht gesund, entfernt der Updater ihn und startet die vorigen Container wieder: Das Update ist **Zurückgesetzt**, und Kaeso läuft mit der Version von vorher. Hat das neue Gateway die Datenbank schon migriert, verweigert das vorige Gateway sie, wie jedes Gateway eine von einer neueren Version migrierte Datenbank verweigert, und das Update ist **Zurücksetzen fehlgeschlagen**. Dann entweder die neue Version von Hand starten und ihr Log lesen (`docker compose logs gateway`) oder die Sicherung wiederherstellen und die alte Version ausführen, wie unter [Downgrade](#downgrade).

Startet der Updater während eines Updates neu, setzt er das Update beim nächsten Start zurück, oder er schließt es ab, wenn nur noch das Aufräumen fehlte.

### Sicherheitsmodell

| Teil | Was er darf |
| --- | --- |
| Gateway | Bittet den Updater, eine Version anhand ihrer Nummer einzuspielen, und liest den Fortschritt. Es bekommt nie Zugriff auf Docker und kann weder Images noch Optionen noch Container nennen. |
| Updater | Nur im internen Netz `updates` erreichbar, das nur das Gateway mit ihm teilt, und nur mit dem Token aus `.env`: mindestens 32 Zeichen, in konstanter Zeit verglichen, nie geloggt oder zurückgegeben. Er ersetzt nur die Container von Gateway und Konsole seines eigenen Compose-Projekts, mit deren eigenen Einstellungen und mit Images, die er über den Digest festgelegt und geprüft hat, und er ändert in der `compose.yaml` nur die Image-Versionen; weicht die Datei in irgendetwas anderem von der des Releases ab, rührt er sie nicht an. |
| `docker-proxy` | [socket-proxy](https://github.com/wollomatic/socket-proxy) im internen Netz `docker`. Er antwortet nur dem Updater und nur auf die Aufrufe der Docker-API, die der Updater macht: Container auflisten und untersuchen, Kaeso-Images untersuchen, laden und taggen, und Container anlegen, starten, stoppen, umbenennen und entfernen. |

Wer Docker steuert, steuert den Server; deshalb ist der Updater optional, läuft ohne alle Capabilities außer der einen, die er zum Schreiben der `compose.yaml` braucht, und teilt diese Netze nur mit dem Gateway und dem Proxy. Ohne das Profil `updater` hat nichts in Kaeso Zugriff auf Docker.

## Während Kaeso aktualisiert

Während der Updater die Versionen umstellt, und immer wenn das Gateway oder die Konsole nicht antwortet, auch bei einem Update von Hand, antwortet Caddy an ihrer Stelle:

| Anfrage | Antwort |
| --- | --- |
| Eine Seite | `503` mit einer Seite auf Englisch, Deutsch oder vereinfachtem Chinesisch, nach der Sprache des Browsers, hell oder dunkel nach dem System. Sie nennt den Schritt, während der Updater arbeitet, prüft alle 5 Sekunden und kehrt zur Konsole zurück, sobald Kaeso wieder da ist. Sie lädt nichts von anderswo. |
| `/platform/v1/*` | `503` mit dem Fehler der Platform API, Typ `unavailable` |
| `/v1/messages` und andere `/v1/*` mit einem Header `anthropic-version` | `529` mit `overloaded_error` von Anthropic, wie das Gateway antwortet, wenn ein Anbieter überlastet ist |
| Andere `/v1/*` | `503` mit `service_unavailable_error` von OpenAI |

Jede Antwort trägt `Retry-After: 30`, das die SDKs von OpenAI und Anthropic beim Wiederholen beachten. Die Meldung sagt, ob Kaeso gerade aktualisiert wird oder gerade nicht erreichbar ist.

## Namen, die bleiben

Diese Namen sind in jedem Release gleich, sodass das Herunterladen der neuen `compose.yaml` und `docker compose pull` die Daten erhalten:

- die Download-Adresse `https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml`;
- die Images `ghcr.io/logicplanes/kaeso-gateway`, `ghcr.io/logicplanes/kaeso-console` und `ghcr.io/logicplanes/kaeso-updater`, mit der Version des Releases als Tag;
- das Compose-Projekt `kaeso` und seine Services `caddy`, `gateway`, `console`, `postgres`, `clickhouse` und `valkey` sowie `updater` und `docker-proxy` im Profil `updater`;
- die Volumes `kaeso_postgres`, `kaeso_clickhouse` und `kaeso_valkey`, die die Daten enthalten, `kaeso_caddy_data` und `kaeso_caddy_config`, die die TLS-Zertifikate enthalten, sowie `kaeso_maintenance` und `kaeso_updater`, die die Wartungsmarkierung und den Fortschritt des Updaters enthalten.

Compose im Ordner der Installation ohne `-p` oder `COMPOSE_PROJECT_NAME` ausführen, damit es den Projektnamen `kaeso` verwendet: Unter einem anderen Projektnamen beginnt Compose mit neuen, leeren Volumes.

## Downgrade

Migrationen lassen sich nicht rückgängig machen. Gegen eine PostgreSQL-Datenbank, die ein neueres Release migriert hat, startet ein Gateway nicht und sagt das. Gegen eine ClickHouse-Datenbank, die ein neueres Release migriert hat, startet es zwar, schreibt aber keine Anfrageprotokolle und protokolliert `ClickHouse migrations failed; request logs are not written`.

Um zu einem älteren Release zurückzukehren, das PostgreSQL-Backup von vor dem Upgrade wiederherstellen, dazu das [ClickHouse-Backup](backups.md#clickhouse), falls das neuere Release ClickHouse migriert hat, und dann die ältere `compose.yaml` starten.
