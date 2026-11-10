---
title: Fly.io
description: Rollen Sie Kaeso auf Fly.io als vier Apps im privaten Netz aus, mit Fly Managed Postgres.
order: 4
---

`deploy/fly/` im Repository enthält eine Fly-Launch-Konfiguration pro App. Fly.io betreibt eine App pro `fly.toml`, und Kaeso braucht vier: einen öffentlichen Edge, ein privates Gateway und eine private Konsole sowie Valkey. Fly Managed Postgres hält die Datenbank. Alle vier Apps sind auf ein Release festgelegt.

## Was die Dateien anlegen

| App | Datei | Image | Maschine |
| --- | --- | --- | --- |
| `…-edge` | `edge.toml` | Das Konsolen-Image, das `kaeso-edge` ausführt | `shared-cpu-1x`, 256 MB |
| `…-gateway` | `gateway.toml` | Das Gateway-Image | `shared-cpu-2x`, 2 GB |
| `…-console` | `console.toml` | Das Konsolen-Image | `shared-cpu-1x`, 256 MB |
| `…-valkey` | `valkey.toml` | Das von Valkey, das Release, das das Compose-Paket betreibt, auf einem Volume mit 1 GB | `shared-cpu-1x`, 512 MB |

Nur der Edge hat einen öffentlichen Dienst. Die anderen Apps haben keinen, sie sind also nur unter `<app>.internal` im privaten Netz der Organisation erreichbar. Dieses Netz ist IPv6, deshalb lauscht das Gateway auf `[::]:8080`. Der Edge liest `Fly-Client-IP`, das Flys Proxy setzt, statt des vom Aufrufer kontrollierten `X-Forwarded-For`. Das Gateway vertraut dem privaten Netz und hält seine Metriken auf seiner Loopback-Adresse.

Valkey hält seine Daten innerhalb von 256 MB und verdrängt nie einen Schlüssel, ein volles Valkey lehnt also Schreibvorgänge ab, statt Limit-Zähler zu verwerfen. Es läuft auf einer Maschine mit einem Volume, das Fly nicht repliziert. Valkey hält nur Zähler und Caches: Das Gateway baut Budget-Zähler aus PostgreSQL neu auf, und Rate-Fenster beginnen von vorn ([Backups und Hauptschlüssel](../backups.md)).

## Ausrollen

Fly.io hat keinen Deploy-Button für ein Projekt aus mehreren Apps, Sie rollen also mit `fly` aus, Flys Kommandozeilenwerkzeug.

1. Erzeugen Sie einen Hauptschlüssel in `KAESO_MASTER_KEYS` und kopieren Sie, was `echo` ausgibt, in einen Passwort-Manager:

   ```bash
   export KAESO_MASTER_KEYS=$(docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate)
   echo $KAESO_MASTER_KEYS
   ```

2. Installieren Sie [`fly`](https://fly.io/docs/flyctl/install/), melden Sie sich an und klonen Sie den Branch `prod`, der das neueste Release enthält:

   ```bash
   fly auth login
   git clone --branch prod https://github.com/logicplanes/kaeso.git
   cd kaeso
   ```

3. App-Namen sind auf ganz Fly.io eindeutig. Wählen Sie also ein eigenes Präfix und legen Sie die vier Apps an:

   ```bash
   export KAESO_APP=kaeso-example
   fly apps create $KAESO_APP-edge
   fly apps create $KAESO_APP-gateway
   fly apps create $KAESO_APP-console
   fly apps create $KAESO_APP-valkey
   ```

4. Legen Sie einen Managed-Postgres-Cluster in der Region der Apps an und hängen Sie ihn an das Gateway. `fly mpg list` zeigt die ID des Clusters:

   ```bash
   fly mpg create --name $KAESO_APP --region fra --pg-major-version 17
   fly mpg attach $CLUSTER_ID --app $KAESO_APP-gateway --variable-name KAESO_POSTGRES_URL
   ```

5. Setzen Sie die Secrets, mit Ihrem SMTP-Server statt der Beispiele ([E-Mail](../email.md)):

   ```bash
   export VALKEY_PASSWORD=$(openssl rand -hex 24)
   fly secrets set --stage --app $KAESO_APP-valkey VALKEY_PASSWORD=$VALKEY_PASSWORD
   fly secrets set --stage --app $KAESO_APP-edge KAESO_GATEWAY_UPSTREAM=$KAESO_APP-gateway.internal:8080 KAESO_CONSOLE_UPSTREAM=$KAESO_APP-console.internal:3000
   fly secrets set --stage --app $KAESO_APP-gateway KAESO_PUBLIC_URL=https://$KAESO_APP-edge.fly.dev \
     KAESO_VALKEY_URL=redis://:$VALKEY_PASSWORD@$KAESO_APP-valkey.internal:6379 KAESO_MASTER_KEYS="$KAESO_MASTER_KEYS" \
     KAESO_SETUP_EMAIL=owner@example.com KAESO_SMTP_HOST=smtp.example.com KAESO_SMTP_USER=kaeso \
     KAESO_SMTP_PASSWORD=secret KAESO_SMTP_FROM="Kaeso <kaeso@example.com>"
   ```

6. Rollen Sie Valkey und die Konsole als je eine Maschine aus, dann das Gateway und den Edge:

   ```bash
   fly deploy --config deploy/fly/valkey.toml --app $KAESO_APP-valkey --ha=false
   fly deploy --config deploy/fly/console.toml --app $KAESO_APP-console --ha=false
   fly deploy --config deploy/fly/gateway.toml --app $KAESO_APP-gateway
   fly deploy --config deploy/fly/edge.toml --app $KAESO_APP-edge
   ```

7. Das Gateway sendet den einmaligen Einrichtungslink an `KAESO_SETUP_EMAIL`; öffnen Sie ihn, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)). Das Gateway-Image hat keine Shell für `kaeso setup-link`. Kommt die E-Mail nicht an, nennen die Logs des Gateways den SMTP-Fehler; korrigieren Sie die Secrets, und das Gateway sendet beim nächsten Start einen neuen Link.
8. Für Ihre eigene Domain fügen Sie dem Edge ein Zertifikat hinzu und richten die DNS-Einträge ein, wie `fly certs add` es angibt. Setzen Sie dann `KAESO_PUBLIC_URL` beim Gateway auf `https://` und die Domain:

   ```bash
   fly certs add kaeso.example.com --app $KAESO_APP-edge
   fly secrets set --app $KAESO_APP-gateway KAESO_PUBLIC_URL=https://kaeso.example.com
   ```

Für Anfrageprotokolle setzen Sie `KAESO_CLICKHOUSE_URL` beim Gateway auf eine ClickHouse-HTTP-URL mit Benutzer und Passwort, etwa ClickHouse Cloud. Ohne sie sind Anfrageprotokolle aus, und Nutzung, Limits und Budgets funktionieren genauso ([Festlegen, wie Anfrageprotokolle gespeichert werden](../installation.md#festlegen-wie-anfrageprotokolle-gespeichert-werden)).

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags in `edge.toml`, `gateway.toml` und `console.toml`. Holen Sie den Branch `prod` und rollen Sie das Gateway aus, dann die Konsole und den Edge, mit den Befehlen aus Schritt 6. Das Gateway migriert die Datenbank beim Start, nur vorwärts ([Upgrades](../upgrades.md)). Eine Maschine bekommt beim Stoppen 75 Sekunden zum Abarbeiten. Zum Zurückrollen rollen Sie ein früheres Image aus, etwa mit `fly deploy --config deploy/fly/gateway.toml --app $KAESO_APP-gateway --image ghcr.io/logicplanes/kaeso-gateway:X.Y.Z`; ein Release, dessen Migrationen die frühere Version nicht ausführen kann, bleibt, wie auf jeder Installation.

## Grenzen

- Fly Machines haben kein schreibgeschütztes Root-Dateisystem und keine Einstellung, um Linux-Capabilities zu entziehen; die Apps laufen also nur mit den Benutzern der Images.
- Managed Postgres bietet PostgreSQL 16 und 17; das Compose-Paket betreibt 18. Kaeso läuft auf 17, wie auf Google Cloud.
- Das Gateway vertraut jeder Adresse im privaten Netz der Organisation. Betreiben Sie Kaeso in einer eigenen Organisation, wenn andere Apps dort die Client-Adresse nicht setzen dürfen, die das Gateway protokolliert.

## Worauf das beruht

Die Dateien folgen Fly.ios Referenz zur App-Konfiguration, seinen Seiten zu Request-Headern und zum privaten Netz sowie den Referenzen zu `fly mpg create` und `fly mpg attach`, gelesen am 9. November 2026. Die vier Dateien lassen sich als TOML lesen und bestehen das [SchemaStore](https://www.schemastore.org/fly.json)-Schema für `fly.toml`, dem der Abschnitt `[checks]` auf oberster Ebene fehlt, den Flys Referenz beschreibt. `fly config validate` braucht ein Fly.io-Konto und lief deshalb nicht. Die Apps wurden noch nicht auf Fly.io ausgerollt. Tun Sie das zuerst in einer Wegwerf-Organisation und prüfen Sie die Einrichtungs-E-Mail, eine gestreamte Antwort und die Client-Adresse in einem Anfrageprotokoll ([was geprüft wurde](index.md#was-geprüft-wurde)).

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Sichern Sie Managed Postgres und bewahren Sie den Hauptschlüssel außerhalb von Fly.io auf.
- [Zustand und Metriken](../monitoring.md): Die Health-Checks, die Edge und Gateway beantworten.
