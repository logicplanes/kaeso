---
title: DigitalOcean
description: Rollen Sie Kaeso mit dem Button Deploy to DO oder aus der App-Spec auf DigitalOcean App Platform aus.
order: 3
---

Zwei [App-Specs für App Platform](https://docs.digitalocean.com/products/app-platform/reference/app-spec/) im Repository betreiben Gateway und Konsole als zwei Dienste hinter dem Edge der App, auf ein Release festgelegt. `.do/deploy.template.yaml` liest der Button **Deploy to DO**: Sie betreibt Valkey in der App. `.do/app.yaml` nutzt stattdessen eine verwaltete Valkey-Datenbank, und Sie legen sie mit `doctl` an.

## Was die Specs anlegen

| Komponente | Art | Image | Größe |
| --- | --- | --- | --- |
| `kaeso-gateway` | Dienst auf Port 8080 | Das Gateway-Image | `apps-s-1vcpu-2gb` |
| `kaeso-console` | Dienst auf Port 3000 | Das Konsolen-Image | `apps-s-1vcpu-0.5gb` |
| `kaeso-postgres` | Dev-Datenbank, PostgreSQL 18 | DigitalOceans | 512 MB |
| `kaeso-valkey`, Button | Dienst auf Port 6379, nur für die Komponenten der App | Valkeys, das Release des Compose-Pakets | `apps-s-1vcpu-0.5gb` |
| `kaeso-valkey`, `doctl` | Verwaltete Valkey-Datenbank | DigitalOceans, vorher angelegt | Ihre Wahl |

Der Edge der App beendet TLS und leitet nach Pfad weiter: `/v1`, `/v1beta`, `/platform`, `/mcp`, `/oauth` und `/.well-known` erreichen das Gateway, jeder andere Pfad die Konsole, hier läuft also kein `kaeso-edge`. Das Gateway vertraut `X-Forwarded-For` aus dem privaten Netz, in dem der Edge sitzt, und hält seine Metriken auf seiner Loopback-Adresse.

Der Button legt nur Dev-Datenbanken an, und die gibt es nur für PostgreSQL. Seine Valkey ist deshalb ein Dienst in der App, der seine Daten in 256 MB hält und nie einen Schlüssel verdrängt. Sie startet bei jedem ihrer Deployments leer. Valkey hält nur Zähler und Caches: Das Gateway baut Budget-Zähler aus PostgreSQL neu auf, und Ratenfenster beginnen von vorn ([Backups und Hauptschlüssel](../backups.md)). Die verwaltete Valkey aus `.do/app.yaml` spricht TLS mit einem Zertifikat von DigitalOceans eigener Stelle, das die Spec dem Gateway als `KAESO_VALKEY_CA` übergibt ([Konfiguration](../configuration.md#einstellungen)).

## Einzutragende Werte

Beide Specs lassen diese Werte leer:

| Wert | Was einzutragen ist |
| --- | --- |
| `KAESO_MASTER_KEYS` | Ein Hauptschlüssel aus `docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate`, auch in einem Passwortmanager aufbewahrt |
| `KAESO_SETUP_EMAIL` | Ihre Adresse; das Gateway schickt den Einrichtungslink dorthin |
| `KAESO_SMTP_HOST`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Ihr SMTP-Server ([E-Mail](../email.md)) |
| `KAESO_CLICKHOUSE_URL` | Eine ClickHouse-HTTP-URL mit Benutzer und Passwort für Anfrageprotokolle, oder leer, um sie auszulassen |

`KAESO_PUBLIC_URL` ist `${APP_URL}`, die Hauptadresse der App, und die Adressen der Datenbanken kommen von den Datenbanken, keine davon wird also getippt. App Platform verschlüsselt einen `SECRET`-Wert, sobald es ihn zum ersten Mal erhält.

## Mit dem Button ausrollen

1. Erzeugen Sie einen Hauptschlüssel und bewahren Sie ihn in einem Passwortmanager auf:

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. Öffnen Sie [Deploy to DO](https://cloud.digitalocean.com/apps/new?repo=https://github.com/logicplanes/kaeso/tree/prod). App Platform liest die Spec aus dem Branch `prod`, der das neueste Release enthält.
3. Tragen Sie in den Umgebungsvariablen von `kaeso-gateway` die [Werte](#einzutragende-werte) ein, und wählen Sie bei Bedarf eine andere Region.
4. Legen Sie die App an und warten Sie, bis ihr erstes Deployment live ist.
5. Das Gateway schickt den einmaligen Einrichtungslink an `KAESO_SETUP_EMAIL`; öffnen Sie ihn, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)).
6. Für Ihre eigene Domain tragen Sie sie in den **Settings** der App unter **Domains** ein und richten den DNS-Eintrag so ein, wie App Platform es sagt. `${APP_URL}` folgt der Hauptdomain, das Gateway rollt also mit der neuen Adresse neu aus.

> [!IMPORTANT]
> Die Dev-Datenbank hält keine Backups. Für die Produktion rollen Sie mit `doctl` aus oder stellen die App wie unten auf einen verwalteten PostgreSQL-Cluster und eine verwaltete Valkey um.

## Mit doctl ausrollen

1. Erzeugen Sie einen Hauptschlüssel wie im ersten Schritt des Buttons.
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

4. Kopieren Sie `.do/app.yaml` aus dem Release, das Sie ausrollen, und tragen Sie die [Werte](#einzutragende-werte) ein.
5. Legen Sie die App an und warten Sie auf ihr erstes Deployment:

   ```bash
   doctl apps create --spec app.yaml --wait
   ```

6. Öffnen Sie den Einrichtungslink aus der E-Mail, um den ersten Benutzer anzulegen.
7. Für Ihre eigene Domain tragen Sie sie unter `domains` in der Spec mit `type: PRIMARY` ein, wenden die Spec mit `doctl apps update` an und richten den DNS-Eintrag so ein, wie der Tab **Networking** der App es sagt.

Die Dev-Datenbank kostet wenig und hält keine Backups. Für die Produktion legen Sie einen verwalteten PostgreSQL-Cluster an und zeigen mit `production: true` und seinem `cluster_name` von `kaeso-postgres` darauf ([Backups und Hauptschlüssel](../backups.md)).

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags von `kaeso-gateway` und `kaeso-console` in beiden Specs. Setzen Sie die beiden Tags in der Spec Ihrer App und wenden Sie sie an, und die App rollt beide Dienste aus; das Gateway migriert die Datenbanken beim Start, nur vorwärts ([Upgrades](../upgrades.md)). `doctl apps spec get $APP_ID` gibt die Spec einer App aus, die der Button angelegt hat:

```bash
doctl apps spec get $APP_ID > app.yaml
doctl apps update $APP_ID --spec app.yaml --wait
```

Zum Zurückrollen wählen Sie **Rollback** neben einem früheren Deployment im Tab **Activity** der App; die zehn jüngsten erfolgreichen Deployments bleiben erhalten. Ein Release, dessen Migrationen die frühere Version nicht ausführen kann, bleibt, wie auf jeder Installation.

## Worauf das beruht

Form der Specs, die Felder für Image, Ingress, Health-Check, Umgebung, interne Ports und Datenbanken sowie die bindbaren `${APP_URL}`, `${…DATABASE_URL}` und `${…CA_CERT}` folgen der App-Spec-Referenz und den Seiten zu Umgebungsvariablen, Container-Images und Datenbanken, gelesen am 7. November 2026. Die Vorlage des Buttons folgt DigitalOceans Seite zum Button Deploy to DO, gelesen am 9. November 2026. An diesem Tag hat die Validierungs- und Proposal-API von App Platform die Spec des Buttons angenommen. Die beiden Images wurden hinter einem pfadweiterleitenden Edge lokal mit Docker getestet ([was geprüft wurde](index.md#was-geprüft-wurde)). Keine der beiden Specs wurde bisher von einem leeren Konto aus auf App Platform ausgerollt. Tun Sie das zuerst in einem Wegwerf-Projekt und prüfen Sie die Einrichtungs-E-Mail und eine gestreamte Antwort, die länger als eine Minute dauert, da der Edge HTTP/1.1 mit dem Gateway spricht. Prüfen Sie auch die Client-Adresse in einem Anfrageprotokoll und ein Update.

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Wechseln Sie zu einem verwalteten PostgreSQL-Cluster mit Backups und bewahren Sie den Hauptschlüssel außerhalb der Plattform auf.
- [Zustand und Metriken](../monitoring.md): Die Health-Checks, die das Gateway beantwortet.
