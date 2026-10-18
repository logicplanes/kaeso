---
title: Upgrades
description: Aktualisieren Sie Kaeso von Hand oder mit einem Klick in der Konsole, und wissen Sie, was während des Updates passiert.
order: 8
---

Jedes Release gibt allen Teilen von Kaeso eine Versionsnummer und folgt Semantic Versioning. Seine `compose.yaml` legt die Images des Releases fest. Aktualisieren Sie von Hand, was immer funktioniert, oder mit einem Klick in der Konsole, sobald Sie den Updater hinzufügen.

## Von Hand aktualisieren

1. Lesen Sie die Release-Notizen auf Schritte, die etwas von Ihnen verlangen. Sie stehen auf der [Release-Seite](https://github.com/logicplanes/kaeso/releases) und unter **Einstellungen** › **Instanz** › **Updates**.
2. [Sichern](backups.md) Sie PostgreSQL und ClickHouse, damit Sie bei Bedarf [zurückgehen](#downgrade) können.
3. Laden Sie im Ordner der Installation die neue `compose.yaml` über die alte. `.env`, `master-keys` und `compose.override.yaml` bleiben, wie sie sind.

```bash
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

4. Laden Sie die neuen Images und starten Sie sie:

```bash
docker compose pull
docker compose up -d --wait
```

Der Befehl kehrt zurück, sobald das neue Gateway und die neue Konsole gesund sind, und scheitert, wenn eines von beiden es nicht ist. Das Gateway migriert die Datenbanken beim Start. Migrationen laufen nur vorwärts und der Reihe nach, sodass Sie Versionen überspringen können.

Wenn ein Update nicht mit einem Klick möglich ist, zeigt die Konsole diese Befehle mit der genauen Version.

## Neue Releases

Das Gateway sucht zweimal am Tag auf GitHub nach neuen Releases, nach einem Fehler frühestens eine Stunde später. Die Anfrage verrät nichts über Ihre Installation außer der Version des Gateways.

Instanz-Admins sehen das Ergebnis unter **Einstellungen** › **Instanz** › **Updates**. Die Seite zeigt die laufende Version, das neueste Release mit den Release-Notizen aller Releases seither und wann das Gateway zuletzt gesucht hat. **Jetzt prüfen** sucht sofort, auch wenn die Suche abgeschaltet ist. Die Markierung **Neu** neben **Updates** ist der einzige Hinweis; Kaeso sendet keine E-Mails oder Banner zu Releases.

| Einstellung | Standard | Was sie bewirkt |
| --- | --- | --- |
| **Nach neuen Releases suchen** | Ein | Sucht zweimal am Tag nach Releases. |
| **Releases** | **Nur stabile** | **Vorabversion** bietet auch Release Candidates an. Eine Installation, die eine Vorabversion ausführt, erhält das stabile Release angeboten, das darauf folgt. |
| **Updates mit einem Klick** | Aus | Erlaubt Instanz-Admins, Updates über den Updater aus der Konsole einzuspielen. |

Auf einer neuen Installation beginnen diese Einstellungen mit `KAESO_UPDATES_CHECK`, `KAESO_UPDATES_CHANNEL` und `KAESO_UPDATES_APPLY` ([Konfiguration](configuration.md#updates)). Danach ändern Instanz-Admins sie auf der Seite, und das Audit-Log der Instanz hält jede Änderung fest.

### Installationen ohne Internetzugang

Schalten Sie vor dem ersten Start die ausgehenden Abfragen des Gateways in der `compose.override.yaml` ab:

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_UPDATES_CHECK: "false"
      KAESO_MODELS_DEV_SYNC: "false"
```

Das Gateway verwendet dann die Modelldaten, die im Release enthalten sind, die Seite **Updates** zeigt die laufende Version, und Sie aktualisieren von Hand. Betreiben Sie einen Spiegel, der wie die Releases-API von GitHub antwortet, richten Sie stattdessen `KAESO_UPDATES_RELEASES_URL` darauf.

## Updates mit einem Klick

Der Updater ist ein optionaler Dienst in der `compose.yaml`, mit einem eigenen Proxy für den Docker-Socket. Er ist der einzige Teil von Kaeso, der mit Docker spricht, und deshalb aus, bis Sie ihn einschalten.

### Updater einschalten

Der Updater braucht Docker Engine 25 oder neuer.

1. Geben Sie im Ordner der Installation Gateway und Updater ein gemeinsames Token und schalten Sie das Profil `updater` ein:

```bash
echo "KAESO_UPDATER_TOKEN=$(openssl rand -hex 32)" >> .env
echo "COMPOSE_PROFILES=updater" >> .env
docker compose up -d --wait
```

2. Schalten Sie **Updates mit einem Klick** unter **Einstellungen** › **Instanz** › **Updates** ein. Das verlangt eine Anmeldung innerhalb der letzten 15 Minuten.

Die Seite sagt, wenn der Updater nicht installiert ist, nicht antwortet oder für ein Release zu alt ist. Liegt der Docker-Socket nicht unter `/var/run/docker.sock`, etwa bei rootless Docker, setzen Sie `DOCKER_SOCKET` in der `.env`.

Um das Token aus der `.env` herauszuhalten, legen Sie es in eine Datei und geben es beiden Diensten als [Compose-Secret](https://docs.docker.com/compose/how-tos/use-secrets/). Beide lesen `KAESO_UPDATER_TOKEN_FILE`:

```yaml title="compose.override.yaml"
services:
  gateway:
    environment:
      KAESO_UPDATER_TOKEN_FILE: /run/secrets/updater_token
    secrets: [updater_token]
  updater:
    environment:
      KAESO_UPDATER_TOKEN_FILE: /run/secrets/updater_token
    secrets: [updater_token]
secrets:
  updater_token:
    file: ./updater-token
```

### Welche Releases sich mit einem Klick einspielen lassen

Ein Release lässt sich mit einem Klick einspielen, wenn alles hier zutrifft. Sonst zeigt die Konsole seine Release-Notizen und die Befehle für das Update von Hand.

- Es ist keine neue Hauptversion. Eine Migration, die einen Schritt von Hand braucht, führt immer zu einer neuen Hauptversion.
- Kein Release auf dem Weg verlangt Schritte von Hand. Das tut ein Release, wenn seine `compose.yaml` sich über die Image-Versionen hinaus ändert oder wenn die Maintainer es so kennzeichnen.
- Es bringt `release.json` mit, das signierte Manifest, das der Updater liest.

### Mit einem Klick aktualisieren

Wählen Sie **Auf … aktualisieren** und bestätigen Sie. Die Konsole verfolgt das Update Schritt für Schritt, und es dauert etwa eine Minute:

1. **Signaturen prüfen.** Der Updater prüft, dass das Release und jedes Image vom Release-Workflow von Kaeso gebaut wurden. Scheitert eine Prüfung, wird nichts eingespielt, und Kaeso läuft weiter.
2. **Herunterladen.** Er lädt die neuen Images von Gateway, Konsole und Updater.
3. **Umstellen.** Er ersetzt erst die Konsole, dann das Gateway durch Container aus den neuen Images, mit denselben Einstellungen. Währenddessen antwortet Caddy an ihrer Stelle ([Während Kaeso aktualisiert wird](#während-kaeso-aktualisiert-wird)).
4. **Migrieren.** Das neue Gateway migriert beim Start die Datenbanken. Der Updater gibt ihm bis zu 10 Minuten, um gesund zu werden.
5. **Wieder erreichbar.** Der Updater schreibt die neuen Versionen in die `compose.yaml`, sodass ein späteres `docker compose up -d --wait` dieselben Images startet.

Der Updater ersetzt sich nicht selbst, während er läuft. Das nächste `docker compose up -d --wait` im Ordner der Installation startet seine neue Version. Es läuft immer nur ein Update, und das Audit-Log der Instanz hält jedes fest.

[Updates](../development/architecture/updates.md) erklärt, wie der Updater ein Release prüft und Container umstellt.

### Ergebnisse

Der Verlauf der Seite zeigt, wie jedes Update ausgegangen ist:

| Ergebnis | Bedeutung |
| --- | --- |
| **Aktualisiert** | Die neue Version läuft. Meldet das Ergebnis, dass der Updater die `compose.yaml` nicht schreiben konnte, setzen Sie die Image-Tags von Kaeso darin selbst auf die neue Version. |
| **Nicht eingespielt** | Nichts hat sich geändert. |
| **Zurückgesetzt** | Die neue Version startete nicht, und die vorherige läuft wieder. |
| **Zurücksetzen fehlgeschlagen** | Die neue Version startete nicht, die vorherige auch nicht. |
| **Prüfung nötig** | Jemand muss nachsehen. Die Details sagen, was passiert ist und was zu tun ist. |

Der Updater setzt selbst zurück, wann immer das sicher geht. Das geht nicht, wenn das neue Gateway scheitert, nachdem es die Datenbanken womöglich migriert hat, denn das vorherige Gateway kann nicht mit ihnen laufen. Dann endet das Update mit **Prüfung nötig**, das neue Gateway und die neue Konsole bleiben, und die vorherigen bleiben angehalten als `kaeso-gateway-1-previous` und `kaeso-console-1-previous`. Wählen Sie einen Weg:

- Finden Sie mit `docker compose logs gateway` heraus, warum das neue Gateway nicht startet, beheben Sie es und setzen Sie die Image-Tags in der `compose.yaml` auf die neue Version.
- Oder stellen Sie das Backup wieder her und führen Sie die alte Version aus, wie unter [Downgrade](#downgrade) beschrieben.

Entfernen Sie danach beide `…-previous`-Container mit `docker rm`. Solange einer existiert, startet der Updater kein weiteres Update.

### Was der Updater darf

Der Updater kann Container anlegen; wer ihn kontrolliert, kontrolliert also den Server als root. Er ist so gebaut, dass diese Befugnis möglichst begrenzt bleibt:

- Er antwortet nur dem Gateway, in einem internen Netzwerk und nur mit dem gemeinsamen Token. Das Gateway übergibt ihm eine Versionsnummer und sonst nichts.
- Er erreicht Docker nur über `docker-proxy`, der nur die Aufrufe durchlässt, die ein Update braucht.
- Er ersetzt nur die Gateway- und Konsolen-Container seines eigenen Compose-Projekts, mit Images, die er geprüft hat.
- Er ändert in der `compose.yaml` nur die Image-Versionen und lässt eine Datei unangetastet, die von der des Releases abweicht.

Ohne das Profil `updater` hat nichts in Kaeso Zugriff auf Docker.

## Während Kaeso aktualisiert wird

Während der Updater die Version umstellt oder wann immer Gateway oder Konsole nicht antworten, antwortet Caddy an ihrer Stelle:

| Anfrage | Antwort |
| --- | --- |
| Eine Konsolenseite | 503 mit einer Seite in der Sprache des Browsers, die sagt, was gerade passiert, alle 5 Sekunden prüft und zurückkehrt, sobald Kaeso wieder da ist |
| `/platform/v1/*` | 503 mit dem Fehler der Platform API, Typ `unavailable` |
| `/v1/messages` und andere `/v1/*` mit einem `anthropic-version`-Header | 529 mit Anthropics `overloaded_error` |
| Andere `/v1/*` | 503 mit OpenAIs `service_unavailable_error` |

Jede Antwort trägt `Retry-After: 30`, das die SDKs von OpenAI und Anthropic bei Wiederholungen beachten.

## Downgrade

Migrationen lassen sich nicht rückgängig machen. Ein Gateway startet nicht mit einer PostgreSQL-Datenbank, die ein neueres Release migriert hat, und sagt das. Mit einer ClickHouse-Datenbank, die ein neueres Release migriert hat, startet es, schreibt aber keine Anfrageprotokolle und protokolliert `ClickHouse migrations failed; request logs are not written`.

So gehen Sie zu einem älteren Release zurück:

1. Stellen Sie das PostgreSQL-Backup von vor dem Upgrade wieder her ([Backups](backups.md#postgresql-wiederherstellen)).
2. Hat das neuere Release ClickHouse migriert, stellen Sie auch ClickHouse wieder her.
3. Laden Sie die `compose.yaml` des älteren Releases und starten Sie sie mit `docker compose up -d --wait`.

## Nächste Schritte

- [Backups und Hauptschlüssel](backups.md): Sichern Sie vor jedem Upgrade.
- [So funktioniert eine Installation](how-it-works.md#namen-die-bleiben): Sehen Sie die Namen, die jedes Release behält, damit ein Upgrade Ihre Daten findet.
