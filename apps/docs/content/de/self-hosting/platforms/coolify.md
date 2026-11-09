---
title: Coolify
description: Rollen Sie Kaeso aus der Compose-Datei auf Coolify aus, mit dem Proxy von Coolify und erzeugten Passwörtern.
order: 8
---

`deploy/coolify/compose.yaml` im Repository ist eine Docker-Compose-Datei für [Coolify](https://coolify.io/docs/knowledge-base/docker/compose). Sie betreibt auf Ihrem Coolify-Server einen öffentlichen Edge hinter dem Proxy von Coolify, ein privates Gateway und eine private Konsole sowie PostgreSQL, ClickHouse und Valkey des Compose-Pakets, alle auf ein Release festgelegt. Coolify erzeugt jedes Passwort und den Hauptschlüssel.

## Was die Datei betreibt

| Dienst | Image | Erreicht von |
| --- | --- | --- |
| `edge` | Das Konsolen-Image, das `kaeso-edge` ausführt | Allen, über den Proxy von Coolify auf der Domain, die Sie ihm geben |
| `gateway` | Das Gateway-Image | Dem Edge |
| `console` | Das Konsolen-Image | Dem Edge |
| `postgres`, `clickhouse`, `valkey` | Die Releases, die das Compose-Paket betreibt | Dem Gateway |

Jeder Dienst behält die Härtung des Compose-Pakets: keine neuen Privilegien, alle Linux-Capabilities entzogen außer denen, die ein Datenspeicher braucht, ein schreibgeschütztes Root-Dateisystem für die Dienste von Kaeso sowie Speicher- und Prozessgrenzen. Das Gateway meldet sich bei PostgreSQL als `kaeso` an, dem seine Datenbank gehört, der aber kein Superuser ist. Valkey verdrängt nie einen Schlüssel, ein volles Valkey lehnt also Schreibvorgänge ab, statt Limit-Zähler zu verwerfen.

Coolify füllt die Werte, deren Namen mit `SERVICE_` beginnen: `SERVICE_URL_EDGE_8080` ist die Adresse des Edge auf Port 8080, die Werte `SERVICE_PASSWORD_…` sind zufällige Passwörter, und `SERVICE_REALBASE64_32_MASTERKEY` sind 32 zufällige Bytes, der Hauptschlüssel.

## Ausrollen

1. Öffnen Sie in Coolify ein Projekt und fügen Sie eine Ressource vom Typ **Docker Compose Empty** hinzu. Fügen Sie die Datei aus dem Branch `prod` ein, der das neueste Release enthält: `https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/coolify/compose.yaml`.
2. Setzen Sie beim Dienst `edge` die Domain auf `https://` und Ihre Domain, etwa `https://kaeso.example.com`, und richten Sie ihren DNS-Eintrag auf den Server. Kaeso braucht eine `https`-Adresse.
3. Tragen Sie unter **Environment Variables** Ihren SMTP-Server ein, falls Sie einen haben ([E-Mail](../email.md)):

   | Variable | Was einzutragen ist |
   | --- | --- |
   | `KAESO_MAIL_TRANSPORT` | `smtp`, um E-Mails zu senden; `log`, der Standard, protokolliert nur die Art jeder E-Mail |
   | `KAESO_SETUP_EMAIL` | Ihre Adresse; mit SMTP sendet das Gateway den Einrichtungslink dorthin |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Ihr SMTP-Server |

4. Rollen Sie aus und warten Sie, bis jeder Dienst gesund ist.
5. Kopieren Sie `SERVICE_REALBASE64_32_MASTERKEY` aus **Environment Variables** in einen Passwort-Manager; der Hauptschlüssel ist `k1:` und dieser Wert. Kaeso verschlüsselt Anbieterschlüssel damit, ein Datenbank-Backup ist ohne ihn also nicht lesbar ([Backups und Hauptschlüssel](../backups.md)).
6. Öffnen Sie den Einrichtungslink, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)). Mit SMTP kommt er an `KAESO_SETUP_EMAIL`. Sonst geben Sie ihn auf dem Server aus, in einem Terminal oder im Terminal von Coolify für den Server:

   ```bash
   docker exec $(docker ps --quiet --filter name=gateway-) kaeso setup-link
   ```

Das Gateway-Image hat keine Shell, das Terminal von Coolify für den Container des Gateways kann den Befehl also nicht ausführen. Auf einem Server mit mehreren Kaeso-Ressourcen geben Sie `docker exec` den Gateway-Container, den `docker ps` für diese hier auflistet.

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags in `compose.yaml`. Öffnen Sie die Compose-Datei der Ressource in Coolify, setzen Sie die Tags von `edge`, `gateway` und `console` auf das neue Release und rollen Sie aus; Coolify behält die erzeugten Werte. Das Gateway migriert die Datenbanken beim Start, nur vorwärts ([Upgrades](../upgrades.md)). Zum Zurückrollen setzen Sie die früheren Tags; ein Release, dessen Migrationen die frühere Version nicht ausführen kann, bleibt, wie auf jeder Installation.

## Grenzen

- Der Updater von Kaeso läuft hier nicht, die Konsole kann Kaeso also nicht aktualisieren; Sie rollen jedes Release in Coolify aus.
- Der Proxy von Coolify terminiert TLS. Das Gateway schreibt in jeden Stream ein Keepalive, lange Streams überdauern also das Leerlauf-Timeout des Proxys ([lange Streams](../domains-and-tls.md#lange-streams)).

## Worauf das beruht

Die Datei folgt den Seiten von Coolify zu Docker Compose und zum Beitragen eines Dienstes sowie seinen magischen Variablen, wie der Quellcode von Coolify sie erzeugt, gelesen am 9. November 2026. `docker compose config` akzeptiert sie. Am 9. November 2026 lief sie lokal mit Docker, mit den Werten, die Coolify erzeugt, von Hand eingetragen: Jeder Dienst wurde gesund, der Edge sandte die APIs an das Gateway und jeden anderen Pfad an die Konsole, `kaeso setup-link` gab den Link aus, und der Benutzer `kaeso` war kein PostgreSQL-Superuser. Sie wurde noch nicht auf Coolify ausgerollt. Tun Sie das zuerst auf einem Wegwerf-Server und prüfen Sie den Einrichtungslink, eine gestreamte Antwort und die Client-Adresse in einem Anfrageprotokoll ([was geprüft wurde](index.md#was-geprüft-wurde)).

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Sichern Sie PostgreSQL und bewahren Sie den Hauptschlüssel außerhalb des Servers auf.
- [Zustand und Metriken](../monitoring.md): Die Health-Checks, die die Dienste beantworten.
