---
title: Backups und Hauptschlüssel
description: Sichern Sie Datenbanken und Hauptschlüssel, stellen Sie sie wieder her und rotieren Sie den Hauptschlüssel.
nav: Backups
order: 9
---

Sichern Sie drei Dinge, bei ausgeschalteten Anfrageprotokollen zwei, und bewahren Sie die Hauptschlüssel getrennt von den Datenbank-Backups auf:

| Was | Enthält | Ohne Backup |
| --- | --- | --- |
| PostgreSQL | Benutzer, Organisationen, Projekte, Schlüssel, verschlüsselte Anbieter-Schlüssel, Kataloge, Limits, Nutzung und Audit-Logs | Die Installation ist verloren |
| ClickHouse | Anfrageprotokolle | Der Protokollverlauf ist verloren; Nutzung, Limits und Budgets hängen nicht davon ab |
| Hauptschlüssel | Die Schlüssel, die gespeicherte Anbieter-Schlüssel verschlüsseln | Jeder Anbieter-Schlüssel muss neu eingegeben werden |

Valkey enthält nur Zähler, Caches und kurzlebige Daten und braucht kein Backup. Das Gateway baut fehlende Budgetzähler aus PostgreSQL neu auf, und Ratenfenster beginnen von vorn.

## PostgreSQL sichern

Sichern Sie die Datenbank, während Kaeso läuft:

```bash
docker compose exec -T postgres \
  pg_dump -U kaeso -Fc kaeso > kaeso-$(date +%F).dump
```

## PostgreSQL wiederherstellen

Halten Sie das Gateway an, stellen Sie den Dump wieder her und starten Sie das Gateway wieder. `--clean --if-exists` ersetzt, was die Datenbank enthält, sodass dieselben Befehle auf einer bestehenden Installation und mit einer leeren Datenbank funktionieren:

```bash
docker compose stop gateway
docker compose exec -T postgres \
  pg_restore -U kaeso -d kaeso --clean --if-exists < kaeso-2026-10-15.dump
docker compose start gateway
```

So stellen Sie auf einem neuen Server wieder her:

1. Legen Sie `compose.yaml`, `.env` und `master-keys` zurück in den Ordner der Installation, und übergeben Sie `master-keys` dem Benutzer des Gateways wie unter [Den Hauptschlüssel erzeugen](../get-started/quick-start.md#den-hauptschlüssel-erzeugen).
2. Starten Sie nur PostgreSQL mit `docker compose up -d --wait postgres`.
3. Stellen Sie den Dump mit dem `pg_restore`-Befehl oben wieder her.
4. Starten Sie den Rest mit `docker compose up -d --wait`.

## ClickHouse sichern

Eine Installation mit ausgeschalteten Anfrageprotokollen hat kein ClickHouse zu sichern, und eine mit Ihrem eigenen ClickHouse sichert es mit dem Rest davon ([festlegen, wie Anfrageprotokolle gespeichert werden](installation.md#festlegen-wie-anfrageprotokolle-gespeichert-werden)).

Für das mitgelieferte ClickHouse verwenden Sie den `BACKUP`-Befehl von ClickHouse mit einem für Backups eingerichteten Datenträger oder ein Werkzeug wie `clickhouse-backup`. Auf einer kleinen Installation können Sie ClickHouse auch anhalten und sein Volume kopieren:

```bash
docker compose stop clickhouse
```

Kopieren Sie das Volume `clickhouse` mit Ihrer üblichen Volume-Sicherung und führen Sie dann `docker compose start clickhouse` aus. Solange ClickHouse nicht läuft, funktionieren Anfragen weiter, und ihre Protokollzeilen warten im Speicher des Gateways, bis zu `KAESO_LOGS_BUFFER_SIZE`.

## Hauptschlüssel sicher aufbewahren

Die Hauptschlüssel sind die Datei `master-keys` neben der `compose.yaml`. Ein gespeicherter Anbieter-Schlüssel vermerkt nur die ID des Hauptschlüssels, der ihn verschlüsselt; ohne seine Hauptschlüssel kann ein Datenbank-Backup also keine Anbieter-Schlüssel entschlüsseln. Bewahren Sie eine Kopie in einem Passwortmanager oder Secret-Store auf, nicht neben den Datenbank-Dumps.

## Hauptschlüssel rotieren

`master-keys` enthält einen oder mehrere Schlüssel, durch Kommas getrennt, den aktiven zuerst: `k7c21e9d04a:…,k3f9a01bc2d:…`. Kaeso verschlüsselt neue Secrets mit dem ersten Schlüssel und entschlüsselt ältere mit ihrem eigenen.

1. Erzeugen Sie einen neuen Schlüssel und schreiben Sie ihn vor den alten. Weil der Befehl in die bestehende Datei schreibt, behält sie ihren Eigentümer und ihre Rechte. Er schreibt nur, wenn beide Schlüssel gelesen wurden, und danach vergisst die Shell beide:

```bash
new=$(docker compose exec -T gateway kaeso master-key generate) &&
  old=$(sudo cat master-keys) &&
  printf '%s,%s\n' "$new" "$old" | sudo tee master-keys > /dev/null
unset new old
```

2. Erstellen Sie das Gateway neu, damit es die neue Datei liest:

```bash
docker compose up -d --wait --force-recreate gateway
```

3. Verschlüsseln Sie jedes gespeicherte Secret mit dem neuen Schlüssel neu:

```bash
docker compose exec gateway kaeso master-key rewrap
```

Der Befehl meldet, wie viele Secrets jede Schlüssel-ID noch verschlüsselt und welche alten IDs nichts mehr verwendet.

4. Entfernen Sie den unbenutzten alten Schlüssel aus `master-keys` und erstellen Sie das Gateway mit dem Befehl aus Schritt 2 neu. Bei mehreren Gateway-Instanzen warten Sie damit, bis jede Instanz mit der neuen Liste läuft.

> [!WARNING]
> Das Gateway startet nicht, solange gespeicherte Secrets eine Schlüssel-ID verwenden, die nicht konfiguriert ist. Entfernen Sie einen Schlüssel nie, bevor `rewrap` ihn als unbenutzt meldet.

## Nächste Schritte

- [Upgrades](upgrades.md): Aktualisieren Sie auf ein neues Release, mit einem Backup, zu dem Sie zurückkehren können.
- [Zustand und Metriken](monitoring.md): Behalten Sie die Installation im Blick.
