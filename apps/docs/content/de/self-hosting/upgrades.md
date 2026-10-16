---
title: Upgrades
description: Eine Installation auf ein neues Release bringen.
order: 6
---

Releases folgen der semantischen Versionierung, und alle Teile von Kaeso tragen die Versionsnummer des Releases. Die Release Notes jedes Releases stehen auf der [Release-Seite](https://github.com/logicplanes/kaeso/releases), und seine `compose.yaml` legt die Gateway- und Konsolen-Images dieses Releases fest.

## Upgrade

1. Die Release Notes auf alles prüfen, was ein Eingreifen erfordert.
2. PostgreSQL [sichern](backups.md).
3. Die `compose.yaml` des neuen Releases über die alte herunterladen. Die eigenen Einstellungen in `.env`, `master-keys` und `compose.override.yaml` bleiben erhalten.

```bash
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
```

4. Die neue Version starten:

```bash
docker compose pull
docker compose up -d
```

Das Gateway migriert PostgreSQL und ClickHouse beim Start unter einem Lock, sodass mehrere Instanzen gleichzeitig starten können und nur eine migriert. Migrationen laufen nur vorwärts und der Reihe nach, sodass ein Upgrade Versionen überspringen kann.

## Downgrade

Ein Gateway startet nicht gegen eine Datenbank, die eine neuere Version migriert hat, und sagt das. Um zu einer älteren Version zurückzukehren, das Backup von vor dem Upgrade wiederherstellen und dann die ältere `compose.yaml` starten.
