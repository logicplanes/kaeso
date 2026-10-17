---
title: Backups und Hauptschlüssel
description: Was gesichert wird, wie es sich wiederherstellen lässt und wie der Hauptschlüssel rotiert wird.
nav: Backups und Schlüssel
order: 7
---

Drei Dinge sichern, und die Hauptschlüssel getrennt von den Datenbank-Backups aufbewahren.

| Was | Enthält | Ohne es |
| --- | --- | --- |
| PostgreSQL | Benutzer, Organisationen, Projekte, Schlüssel, verschlüsselte Anbieter-Schlüssel, Kataloge, Limits, Nutzungsdaten, Audit-Logs | Die Installation ist verloren |
| ClickHouse | Anfrageprotokolle und Nutzungsaggregate | Der Protokollverlauf ist verloren; Limits und Budgets hängen nicht davon ab |
| Hauptschlüssel | Die Schlüssel, die gespeicherte Anbieter-Schlüssel verschlüsseln | Anbieter-Schlüssel müssen neu eingegeben werden |

Valkey enthält nur Zähler und Caches. Jedes Limit hat eigene Zähler; Budgetzähler werden aus PostgreSQL neu aufgebaut, wenn sie fehlen, und Ratenfenster beginnen von vorn.

## PostgreSQL

Die Datenbank dumpen, während Kaeso läuft:

```bash
docker compose exec -T postgres pg_dump -U kaeso -Fc kaeso > kaeso-$(date +%F).dump
```

Bei angehaltenem Gateway wiederherstellen. `--clean --if-exists` ersetzt, was die Datenbank enthält, sodass derselbe Befehl bei einer laufenden Installation und bei einer leeren Datenbank funktioniert:

```bash
docker compose stop gateway
docker compose exec -T postgres pg_restore -U kaeso -d kaeso --clean --if-exists < kaeso-2026-10-15.dump
docker compose start gateway
```

Auf einem neuen Server zuerst `compose.yaml`, `.env` und `master-keys` zurücklegen, nur PostgreSQL mit `docker compose up -d --wait postgres` starten, wiederherstellen und dann den Rest mit `docker compose up -d --wait` starten.

## ClickHouse

Den Befehl `BACKUP` von ClickHouse auf eine für Backups konfigurierte Disk nutzen, oder ein Tool wie `clickhouse-backup`. Bei einer kleinen Installation funktioniert es auch, ClickHouse anzuhalten und sein Volume zu kopieren:

```bash
docker compose stop clickhouse
```

Das Volume `clickhouse` mit dem üblichen Volume-Backup kopieren und es dann mit `docker compose start clickhouse` wieder starten. Solange ClickHouse nicht läuft, funktionieren Anfragen weiter, und Protokollzeilen warten im Speicher des Gateways, bis zu `KAESO_LOGS_BUFFER_SIZE`.

## Hauptschlüssel

Die Hauptschlüssel sind die Datei `master-keys` neben `compose.yaml`, die das Gateway als Docker-Secret `master_keys` liest. Jeder gespeicherte Anbieter-Schlüssel vermerkt die ID des Hauptschlüssels, der ihn verschlüsselt, nie den Schlüssel selbst, sodass ein Datenbank-Backup ohne seinen Hauptschlüssel keine Anbieter-Schlüssel entschlüsseln kann.

Eine Kopie in einem Passwortmanager oder einem Secret-Store aufbewahren, nicht neben den Datenbank-Dumps.

## Den Hauptschlüssel rotieren

`master-keys` enthält einen oder mehrere Schlüssel, durch Kommas getrennt, den aktiven Schlüssel zuerst: `k7c21e9d04a:…,k3f9a01bc2d:…`. Neue Secrets nutzen den ersten Schlüssel; ältere lassen sich weiter mit ihrem entschlüsseln.

1. Einen neuen Schlüssel erzeugen und ihn an den Anfang setzen, vor den alten:

```bash
printf '%s,%s\n' "$(docker compose exec -T gateway kaeso master-key generate)" "$(cat master-keys)" > master-keys.new
mv master-keys.new master-keys
chmod 644 master-keys
```

2. Das Gateway neu erstellen, damit es die neue Datei liest:

```bash
docker compose up -d --wait --force-recreate gateway
```

3. Jedes gespeicherte Secret mit dem neuen Schlüssel neu verschlüsseln:

```bash
docker compose exec gateway kaeso master-key rewrap
```

Der Befehl meldet, wie viele Secrets jede Schlüssel-ID noch verschlüsselt und welche alten IDs nichts mehr nutzt.

4. Wird die alte ID nicht mehr genutzt, sie aus `master-keys` entfernen und das Gateway mit demselben Befehl neu erstellen. Bei mehreren Gateway-Instanzen sie erst entfernen, wenn jede Instanz mit der neuen Liste läuft.

> [!WARNING]
> Das Gateway startet nicht, wenn gespeicherte Secrets eine Schlüssel-ID nutzen, die nicht konfiguriert ist, und nennt die ID. Nie einen Schlüssel entfernen, bevor `rewrap` ihn als ungenutzt meldet.
