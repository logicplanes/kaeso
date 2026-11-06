---
title: Railway
description: Rollen Sie Kaeso aus der Infrastrukturdatei auf Railway aus, mit Railway Postgres und Redis.
order: 2
---

`.railway/railway.ts` im Repository beschreibt ein Railway-Projekt als [Infrastructure as Code](https://docs.railway.com/infrastructure-as-code): einen öffentlichen Edge, ein privates Gateway und eine private Konsole, einen Postgres-Dienst und einen Redis-Dienst, alle auf ein Release festgelegt. Railways ältere `railway.json` und `railway.toml` funktionieren ab dem 1. Dezember 2026 nicht mehr, das Repository hat also keine.

## Was die Datei anlegt

| Dienst | Image | Erreichbar für |
| --- | --- | --- |
| `kaeso-edge` | Das Konsolen-Image, das `kaeso-edge` ausführt | Alle, auf der Domain, die Sie ihm geben |
| `kaeso-gateway` | Das Gateway-Image | Den Edge, über Railways privates Netz |
| `kaeso-console` | Das Konsolen-Image | Den Edge, über Railways privates Netz |
| `kaeso-postgres` | Railway Postgres | Das Gateway |
| `kaeso-valkey` | Railway Redis | Das Gateway |

Railways privates Netz ist reines IPv6, das Gateway lauscht also auf `[::]:8080` und vertraut dem Edge aus dem Unique-Local-Adressbereich. Jedes Image hat automatische Updates aus, die Dienste bleiben also auf dem Release, das die Datei nennt.

## Ausrollen

1. Erzeugen Sie einen Hauptschlüssel und bewahren Sie ihn in einem Passwort-Manager auf:

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. Installieren Sie die Railway-CLI, melden Sie sich an und legen Sie ein leeres Projekt an:

   ```bash
   railway login
   railway init
   ```

3. Fügen Sie in den **Settings** des Projekts unter **Shared Variables** diese hinzu:

   | Variable | Was einzutragen ist |
   | --- | --- |
   | `KAESO_PUBLIC_URL` | `https://` und die Domain des Edge; tragen Sie jetzt einen Platzhalter ein und korrigieren Sie ihn in Schritt 6 |
   | `KAESO_MASTER_KEYS` | Der Schlüssel aus Schritt 1 |
   | `KAESO_SETUP_EMAIL` | Ihre Adresse; das Gateway sendet den Einrichtungslink dorthin |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Ihr SMTP-Server ([E-Mail](../email.md)) |
   | `KAESO_CLICKHOUSE_URL` | Eine ClickHouse-HTTP-URL mit Benutzer und Passwort für Anfrageprotokolle, oder leer, um sie aus zu lassen |

4. Legen Sie die `.railway/railway.ts` des Releases in einen eigenen Ordner, von `https://github.com/logicplanes/kaeso/blob/v{{version}}/.railway/railway.ts`, und wenden Sie sie aus diesem Ordner an:

   ```bash
   railway config apply
   ```

5. Geben Sie `kaeso-edge` eine Domain: Erzeugen Sie unter seinen **Settings** eine Railway-Domain oder fügen Sie Ihre eigene hinzu, für Port 8080.
6. Setzen Sie `KAESO_PUBLIC_URL` auf `https://` und diese Domain und rollen Sie `kaeso-gateway` neu aus. Das Gateway sendet den einmaligen Einrichtungslink an `KAESO_SETUP_EMAIL`; öffnen Sie ihn, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)).

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags in `railway.ts`. Wenden Sie die neue Datei genauso an; Railway rollt die geänderten Dienste neu aus, und das Gateway migriert die Datenbanken beim Start, nur vorwärts ([Upgrades](../upgrades.md)). Zum Zurückrollen rollen Sie ein früheres Deployment eines Dienstes aus seiner Liste **Deployments** neu aus.

## Worauf das beruht

Die Datei folgt Railways TypeScript-Referenz, gelesen am 6. November 2026: `defineRailway`, `project`, `service`, `image`, `postgres`, `redis`, Dienstumgebungen aus `db.env` und `cache.env`, geteilte Variablen über `ctx.shared` und `${{service.RAILWAY_PRIVATE_DOMAIN}}` für die privaten Adressen. Railway kennzeichnet die Funktion als experimentell, ihre API kann sich also noch ändern. Die drei Images wurden in diesem Aufbau lokal mit Docker getestet ([was geprüft wurde](index.md#was-geprüft-wurde)). Die Datei wurde noch nicht von einem leeren Projekt aus auf Railway angewendet; tun Sie das zuerst und prüfen Sie die privaten DNS-Namen, die Einrichtungs-E-Mail, eine gestreamte Antwort und ein Update.

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Sichern Sie Postgres auf Railway und bewahren Sie den Hauptschlüssel außerhalb auf.
- [Zustand und Metriken](../monitoring.md): Die Health-Checks, die Edge und Gateway beantworten.
