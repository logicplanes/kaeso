---
title: Render
description: Rollen Sie Kaeso aus dem Blueprint auf Render aus, mit Render Postgres und Key Value.
order: 1
---

`render.yaml` im Wurzelverzeichnis des Repositorys ist ein [Render Blueprint](https://render.com/docs/infrastructure-as-code). Er legt einen öffentlichen Edge, ein privates Gateway und eine private Konsole, eine Postgres-Instanz und eine Key-Value-Instanz an, alle auf ein Release festgelegt.

## Was der Blueprint anlegt

| Dienst | Typ | Image | Plan |
| --- | --- | --- | --- |
| `kaeso-edge` | Web Service | Das Konsolen-Image, das `kaeso-edge` ausführt | 0,5 CPU, 512 MB |
| `kaeso-gateway` | Private Service | Das Gateway-Image | 2 CPU, 4 GB |
| `kaeso-console` | Private Service | Das Konsolen-Image | 0,5 CPU, 512 MB |
| `kaeso-valkey` | Key Value | Renders, mit `noeviction` | 256 MB |
| `kaeso-postgres` | Postgres | Renders, Version 18 | 1 CPU, 2 GB |

Nur der Edge ist öffentlich. Renders privates Netz trägt Anfragen vom Edge zum Gateway und zur Konsole und vom Gateway zu Postgres und Key Value, deren `ipAllowList` leer ist. Jeder Dienst hat `autoDeployTrigger: off`, Render rollt also nie bei einem Commit neu aus. Ändern Sie einen Plan oder die Region im Blueprint, bevor Sie ihn anlegen; Render behält sie danach.

## Ausrollen

1. Erzeugen Sie einen Hauptschlüssel und bewahren Sie ihn in einem Passwort-Manager auf:

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. Legen Sie in Render einen **Blueprint** aus dem Repository `logicplanes/kaeso`, Branch `prod`, an, der das neueste Release enthält. Render zeigt die Werte, die der Blueprint abfragt:

   | Wert | Was einzutragen ist |
   | --- | --- |
   | `KAESO_PUBLIC_URL` | `https://` und die Adresse des Edge, etwa `https://kaeso-edge.onrender.com`, oder Ihre Domain, sobald sie eingerichtet ist |
   | `KAESO_MASTER_KEYS` | Der Schlüssel aus Schritt 1 |
   | `KAESO_SETUP_EMAIL` | Ihre Adresse; das Gateway sendet den Einrichtungslink dorthin |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Ihr SMTP-Server ([E-Mail](../email.md)) |
   | `KAESO_CLICKHOUSE_URL` | Eine ClickHouse-HTTP-URL mit Benutzer und Passwort für Anfrageprotokolle, oder leer, um sie aus zu lassen |

3. Warten Sie, bis jeder Dienst läuft. Das Gateway sendet den einmaligen Einrichtungslink an `KAESO_SETUP_EMAIL`; öffnen Sie ihn, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)).
4. Für Ihre eigene Domain fügen Sie sie bei `kaeso-edge` unter **Settings** hinzu, richten den DNS-Eintrag ein, wie Render es angibt, setzen `KAESO_PUBLIC_URL` bei `kaeso-gateway` auf `https://` und die Domain und rollen das Gateway neu aus.

Das Gateway lauscht auf Port 8080 und hält seine Metriken auf seiner Loopback-Adresse, denn Render nimmt den einen Port, den ein Private Service auf allen Schnittstellen bindet, als seinen Port. Der Edge vertraut `X-Forwarded-For` aus Renders privatem Netz, in dem Renders eigener Proxy sitzt, und das Gateway vertraut dem Edge.

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags in `render.yaml`. Auf der Seite des Blueprints rollt **Manual sync** die Dienste auf dem neuen Release neu aus; das Gateway migriert die Datenbanken beim Start, nur vorwärts ([Upgrades](../upgrades.md)). Zum Zurückrollen nutzen Sie **Rollback** bei jedem Dienst auf sein vorheriges Deployment; ein Release, dessen Migrationen die frühere Version nicht ausführen kann, bleibt, wie auf jeder Installation.

## Worauf das beruht

Form des Blueprints, Diensttypen, Plannamen, die Verweise `fromService` und `fromDatabase` sowie `ipAllowList` folgen Renders Blueprint-Spezifikation, gelesen am 6. November 2026. Die drei Images wurden in diesem Aufbau lokal mit Docker getestet ([was geprüft wurde](index.md#was-geprüft-wurde)). Der Blueprint wurde noch nicht von einem leeren Konto aus auf Render ausgerollt; tun Sie das zuerst in einem Wegwerf-Workspace und prüfen Sie die Einrichtungs-E-Mail, eine gestreamte Antwort, die Client-Adresse in einem Anfrageprotokoll und ein Update.

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Sichern Sie Postgres auf Render und bewahren Sie den Hauptschlüssel außerhalb auf.
- [Zustand und Metriken](../monitoring.md): Die Health-Checks, die Edge und Gateway beantworten.
