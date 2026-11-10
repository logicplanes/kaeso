---
title: Dokploy
description: Rollen Sie Kaeso auf Dokploy aus, indem Sie die Vorlage importieren, mit Traefik und erzeugten Passwörtern.
order: 9
---

`deploy/dokploy/` im Repository enthält eine [Dokploy-Vorlage](https://github.com/Dokploy/templates): `docker-compose.yml` und `template.toml`. Sie betreibt auf Ihrem Dokploy-Server einen öffentlichen Edge hinter dem Traefik von Dokploy, ein privates Gateway und eine private Konsole sowie PostgreSQL, ClickHouse und Valkey des Compose-Pakets, alle auf ein Release festgelegt. Die Vorlage erzeugt jedes Passwort und den Hauptschlüssel.

## Was die Vorlage betreibt

| Dienst | Image | Erreicht von |
| --- | --- | --- |
| `edge` | Das Konsolen-Image, das `kaeso-edge` ausführt | Allen, über Traefik auf der Domain der Vorlage |
| `gateway` | Das Gateway-Image | Dem Edge |
| `console` | Das Konsolen-Image | Dem Edge |
| `postgres`, `clickhouse`, `valkey` | Die Releases, die das Compose-Paket betreibt | Dem Gateway |

Jeder Dienst behält die Härtung des Compose-Pakets: keine neuen Privilegien, alle Linux-Capabilities entzogen außer denen, die ein Datenspeicher braucht, ein schreibgeschütztes Root-Dateisystem für die Dienste von Kaeso sowie Speicher- und Prozessgrenzen. Das Gateway meldet sich bei PostgreSQL als `kaeso` an, dem seine Datenbank gehört, der aber kein Superuser ist. Valkey verdrängt nie einen Schlüssel, ein volles Valkey lehnt also Schreibvorgänge ab, statt Limit-Zähler zu verwerfen.

`template.toml` gibt dem Edge eine Domain auf Port 8080 und setzt die Umgebung: zufällige Passwörter aus dem Helfer `hash` von Dokploy und einen Hauptschlüssel aus seinem Helfer `base64`, 32 zufällige Bytes. E-Mails werden nur protokolliert, bis Sie einen SMTP-Server eintragen.

## Ausrollen

1. Wandeln Sie auf einem Rechner mit `curl` und `jq` die Vorlage aus dem Branch `prod`, der das neueste Release enthält, in den Base64-Text um, den Dokploy importiert:

   ```bash
   curl -fsSLO https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/dokploy/docker-compose.yml
   curl -fsSLO https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/dokploy/template.toml
   jq -n --rawfile compose docker-compose.yml --rawfile config template.toml '{compose: $compose, config: $config}' | base64 | tr -d '\n'
   ```

2. Öffnen Sie in Dokploy ein Projekt und legen Sie einen Dienst vom Typ **Compose** an. Fügen Sie unter **Advanced** den Text in **Import** ein und importieren Sie ihn. Dokploy zeigt die Compose-Datei, die Umgebung, die Mounts und die Domain, bevor es sie übernimmt.
3. Ändern Sie unter **Domains** den Host auf Ihre Domain, etwa `kaeso.example.com`, schalten Sie HTTPS mit Let's Encrypt ein und richten Sie den DNS-Eintrag der Domain auf den Server.
4. Setzen Sie unter **Environment** `KAESO_PUBLIC_URL` auf `https://` und diese Domain, und tragen Sie Ihren SMTP-Server ein, falls Sie einen haben ([E-Mail](../email.md)):

   | Variable | Was einzutragen ist |
   | --- | --- |
   | `KAESO_MAIL_TRANSPORT` | `smtp`, um E-Mails zu senden; `log`, der Standard, protokolliert nur die Art jeder E-Mail |
   | `KAESO_SETUP_EMAIL` | Ihre Adresse; mit SMTP sendet das Gateway den Einrichtungslink dorthin |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Ihr SMTP-Server |

5. Rollen Sie aus und warten Sie, bis jeder Dienst gesund ist.
6. Kopieren Sie `KAESO_MASTER_KEYS` aus **Environment** in einen Passwort-Manager. Kaeso verschlüsselt Anbieterschlüssel damit, ein Datenbank-Backup ist ohne ihn also nicht lesbar ([Backups und Hauptschlüssel](../backups.md)).
7. Öffnen Sie den Einrichtungslink, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)). Mit SMTP kommt er an `KAESO_SETUP_EMAIL`. Sonst geben Sie ihn auf dem Server aus, mit dem App-Namen des Dienstes aus seinem Reiter **General** statt `kaeso-example`:

   ```bash
   docker exec kaeso-example-gateway-1 kaeso setup-link
   ```

Das Gateway-Image hat keine Shell, das Terminal von Dokploy für den Container des Gateways kann den Befehl also nicht ausführen.

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags in `docker-compose.yml`. Öffnen Sie die Compose-Datei des Dienstes in Dokploy, setzen Sie die Tags von `edge`, `gateway` und `console` auf das neue Release und rollen Sie aus; die Umgebung bleibt. Das Gateway migriert die Datenbanken beim Start, nur vorwärts ([Upgrades](../upgrades.md)). Zum Zurückrollen setzen Sie die früheren Tags; ein Release, dessen Migrationen die frühere Version nicht ausführen kann, bleibt, wie auf jeder Installation.

## Grenzen

- Der Updater von Kaeso läuft hier nicht, die Konsole kann Kaeso also nicht aktualisieren; Sie rollen jedes Release in Dokploy aus.
- Traefik terminiert TLS. Das Gateway schreibt in jeden Stream ein Keepalive, lange Streams überdauern also das Leerlauf-Timeout des Proxys ([lange Streams](../domains-and-tls.md#lange-streams)).

## Worauf das beruht

Die Vorlage folgt dem Format des Vorlagen-Repositorys von Dokploy und dem Quellcode von Dokploy für den Import einer Vorlage und für seine Helfer, gelesen am 9. November 2026. `docker compose config` akzeptiert die Compose-Datei mit der Umgebung der Vorlage. Am 9. November 2026 lief sie lokal mit Docker, mit dieser Umgebung von Hand eingetragen: Jeder Dienst wurde gesund, der Edge sandte die APIs an das Gateway und jeden anderen Pfad an die Konsole, das Gateway migrierte ClickHouse, `kaeso setup-link` gab den Link aus, und der Benutzer `kaeso` war kein PostgreSQL-Superuser. Sie wurde noch nicht in Dokploy importiert. Tun Sie das zuerst auf einem Wegwerf-Server und prüfen Sie den Einrichtungslink, eine gestreamte Antwort und die Client-Adresse in einem Anfrageprotokoll ([was geprüft wurde](index.md#was-geprüft-wurde)).

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Sichern Sie PostgreSQL und bewahren Sie den Hauptschlüssel außerhalb des Servers auf.
- [Zustand und Metriken](../monitoring.md): Die Health-Checks, die die Dienste beantworten.
