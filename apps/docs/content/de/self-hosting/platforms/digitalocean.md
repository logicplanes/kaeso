---
title: DigitalOcean
description: Rollen Sie Kaeso aus der App-Spec auf DigitalOcean App Platform aus, mit einer Dev-PostgreSQL und einer verwalteten Valkey.
order: 3
---

`.do/app.yaml` im Repository ist eine [App-Spec für App Platform](https://docs.digitalocean.com/products/app-platform/reference/app-spec/). Sie betreibt Gateway und Konsole als zwei Dienste hinter dem Edge der App, mit einer Dev-PostgreSQL-Datenbank und einer verwalteten Valkey-Datenbank, alle auf ein Release festgelegt.

## Was die Spec anlegt

| Komponente | Art | Image | Größe |
| --- | --- | --- | --- |
| `kaeso-gateway` | Dienst auf Port 8080 | Das Gateway-Image | `apps-s-1vcpu-2gb` |
| `kaeso-console` | Dienst auf Port 3000 | Das Konsolen-Image | `apps-s-1vcpu-0.5gb` |
| `kaeso-postgres` | Dev-Datenbank, PostgreSQL 18 | DigitalOceans | 512 MB |
| `kaeso-valkey` | Verwaltete Valkey-Datenbank | DigitalOceans, vorher angelegt | Ihre Wahl |

Der Edge der App beendet TLS und leitet nach Pfad weiter: `/v1`, `/platform`, `/mcp`, `/oauth` und `/.well-known` erreichen das Gateway, jeder andere Pfad die Konsole, hier läuft also kein `kaeso-edge`. Das Gateway vertraut `X-Forwarded-For` aus dem privaten Netz, in dem der Edge sitzt, und hält seine Metriken auf seiner Loopback-Adresse. Die verwaltete Valkey spricht TLS mit einem Zertifikat von DigitalOceans eigener Stelle, das die Spec dem Gateway als `KAESO_VALKEY_CA` übergibt ([Konfiguration](../configuration.md#einstellungen)).

Der Button **Deploy to DO** rollt einen Dienst mit einer Dev-Datenbank aus und sonst nichts; Kaeso, das zwei Dienste und eine Valkey braucht, wird deshalb mit `doctl` angelegt.

## Ausrollen

1. Erzeugen Sie einen Hauptschlüssel und bewahren Sie ihn in einem Passwortmanager auf:

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. Installieren Sie [`doctl`](https://docs.digitalocean.com/reference/doctl/), melden Sie sich an und legen Sie die Valkey-Datenbank in der Region der App an:

   ```bash
   doctl auth init
   doctl databases create kaeso-valkey --engine valkey --region fra1 --size db-s-1vcpu-1gb --num-nodes 1 --wait
   ```

3. Setzen Sie in den **Settings** der Datenbank im Control Panel die **Eviction policy** auf **No eviction**, damit eine volle Datenbank Schreibzugriffe ablehnt, statt Limit-Zähler zu verwerfen. Dasselbe über die API, mit der Id der Datenbank:

   ```bash
   curl -X PUT -H "Authorization: Bearer $DIGITALOCEAN_TOKEN" -H "Content-Type: application/json" \
     -d '{"eviction_policy":"noeviction"}' "https://api.digitalocean.com/v2/databases/$DATABASE_ID/eviction_policy"
   ```

4. Kopieren Sie `.do/app.yaml` aus dem Release, das Sie ausrollen, und füllen Sie die Werte aus, die sie leer lässt:

   | Wert | Was einzutragen ist |
   | --- | --- |
   | `KAESO_MASTER_KEYS` | Der Schlüssel aus Schritt 1 |
   | `KAESO_SETUP_EMAIL` | Ihre Adresse; das Gateway schickt den Einrichtungslink dorthin |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Ihr SMTP-Server ([E-Mail](../email.md)) |
   | `KAESO_CLICKHOUSE_URL` | Eine ClickHouse-HTTP-URL mit Benutzer und Passwort für Anfrageprotokolle, oder leer, um sie auszulassen |

   `KAESO_PUBLIC_URL` ist `${APP_URL}`, die Hauptadresse der App, und die Adressen der Datenbanken kommen von den Datenbanken, keine davon wird also getippt. App Platform verschlüsselt einen `SECRET`-Wert, sobald es ihn zum ersten Mal erhält.

5. Legen Sie die App an und warten Sie auf ihr erstes Deployment:

   ```bash
   doctl apps create --spec app.yaml --wait
   ```

6. Das Gateway schickt den einmaligen Einrichtungslink an `KAESO_SETUP_EMAIL`; öffnen Sie ihn, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)).
7. Für Ihre eigene Domain tragen Sie sie unter `domains` in der Spec mit `type: PRIMARY` ein, wenden die Spec mit `doctl apps update` an und richten den DNS-Eintrag so ein, wie der Tab **Networking** der App es sagt. `${APP_URL}` folgt der Hauptdomain, das Gateway rollt also mit der neuen Adresse neu aus.

Die Dev-Datenbank kostet wenig und hält keine Backups. Für die Produktion legen Sie einen verwalteten PostgreSQL-Cluster an und zeigen mit `production: true` und seinem `cluster_name` von `kaeso-postgres` darauf ([Backups und Hauptschlüssel](../backups.md)).

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags in `.do/app.yaml`. Wenden Sie die neue Spec an, und die App rollt beide Dienste aus; das Gateway migriert die Datenbanken beim Start, nur vorwärts ([Upgrades](../upgrades.md)):

```bash
doctl apps update $APP_ID --spec app.yaml --wait
```

Zum Zurückrollen wählen Sie **Rollback** neben einem früheren Deployment im Tab **Activity** der App; die zehn jüngsten erfolgreichen Deployments bleiben erhalten. Ein Release, dessen Migrationen die frühere Version nicht ausführen kann, bleibt, wie auf jeder Installation.

## Worauf das beruht

Form der Spec, die Felder für Image, Ingress, Health-Check, Umgebung und Datenbanken sowie die bindbaren `${APP_URL}`, `${…DATABASE_URL}` und `${…CA_CERT}` folgen der App-Spec-Referenz und den Seiten zu Umgebungsvariablen, Container-Images und Datenbanken, gelesen am 7. November 2026. Die beiden Images wurden hinter einem pfadweiterleitenden Edge lokal mit Docker getestet ([was geprüft wurde](index.md#was-geprüft-wurde)). Die Spec wurde noch nicht von einem leeren Konto aus auf App Platform ausgerollt; tun Sie das zuerst in einem Wegwerf-Projekt und prüfen Sie die Einrichtungs-E-Mail, eine gestreamte Antwort, die länger als eine Minute dauert, da der Edge HTTP/1.1 mit dem Gateway spricht, die Client-Adresse in einem Anfrageprotokoll und ein Update.

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Wechseln Sie zu einem verwalteten PostgreSQL-Cluster mit Backups und bewahren Sie den Hauptschlüssel außerhalb der Plattform auf.
- [Zustand und Metriken](../monitoring.md): Die Health-Checks, die das Gateway beantwortet.
