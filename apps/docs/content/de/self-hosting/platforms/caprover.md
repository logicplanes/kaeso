---
title: CapRover
description: Rollen Sie Kaeso als One-Click-App aus der Vorlage im Repository auf CapRover aus.
order: 10
---

`deploy/caprover/kaeso.yml` im Repository ist eine [One-Click-App für CapRover](https://github.com/caprover/one-click-apps). Sie legt auf Ihrem CapRover-Server sechs Apps an: einen öffentlichen Edge hinter dem nginx von CapRover, ein privates Gateway und eine private Konsole sowie PostgreSQL, ClickHouse und Valkey des Compose-Pakets, alle auf ein Release festgelegt.

## Was die Vorlage anlegt

| App | Image | Erreicht von |
| --- | --- | --- |
| `kaeso` | Das Konsolen-Image, das `kaeso-edge` ausführt | Allen, über nginx auf der Domain der App |
| `kaeso-gateway` | Das Gateway-Image | Dem Edge |
| `kaeso-console` | Das Konsolen-Image | Dem Edge |
| `kaeso-postgres`, `kaeso-clickhouse`, `kaeso-valkey` | Die Releases, die das Compose-Paket betreibt | Dem Gateway |

Die Namen folgen dem App-Namen, den Sie angeben, hier `kaeso`. Nur der Edge ist eine Web-App; die anderen sind nur im Overlay-Netz von CapRover erreichbar. Das Gateway meldet sich bei PostgreSQL als `kaeso` an, dem seine Datenbank gehört, der aber kein Superuser ist. Valkey hält seine Daten innerhalb von 512 MB und verdrängt nie einen Schlüssel, ein volles Valkey lehnt also Schreibvorgänge ab, statt Limit-Zähler zu verwerfen.

Der nginx von CapRover hängt an das `X-Forwarded-For` des Aufrufers an, deshalb liest der Edge `X-Real-IP`, das nginx setzt. Die Vorlage schaltet für den Edge die Websocket-Unterstützung ein, mit der nginx HTTP/1.1 zu ihm spricht. Das Gateway vertraut nur privaten Adressen und hält seine Metriken auf seiner Loopback-Adresse.

## Ausrollen

1. Erzeugen Sie einen Hauptschlüssel und bewahren Sie ihn in einem Passwort-Manager auf:

   ```bash
   docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate
   ```

2. Öffnen Sie in CapRover **Apps**, dann **One-Click Apps/Databases**, und wählen Sie **>> TEMPLATE <<** am Ende der Liste.
3. Fügen Sie die Vorlage aus dem Branch `prod` ein, der das neueste Release enthält: `https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/caprover/kaeso.yml`.
4. Füllen Sie das Formular aus:

   | Feld | Was einzutragen ist |
   | --- | --- |
   | App name | `kaeso` oder ein eigener Name |
   | Public URL | Die Adresse der App unter Ihrer Root-Domain, die das Formular einträgt, oder Ihre eigene Domain, etwa `https://kaeso.example.com` |
   | Master key | Der Schlüssel aus Schritt 1 |
   | Passwörter | Zufällige, die das Formular einträgt |
   | Mail transport | `smtp`, um E-Mails zu senden; `log`, der Standard, protokolliert nur die Art jeder E-Mail |
   | Setup email, SMTP host, port, user, password, sender | Ihr SMTP-Server, falls Sie einen haben ([E-Mail](../email.md)) |

5. Rollen Sie aus und warten Sie, bis die Apps angelegt sind. Das Gateway startet neu, bis PostgreSQL und Valkey antworten.
6. Öffnen Sie die App `kaeso`, schalten Sie **HTTPS** und **Force HTTPS** ein und verbinden Sie Ihre Domain, falls die Public URL eine nennt.
7. Öffnen Sie den Einrichtungslink, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)). Mit SMTP kommt er an die Setup-E-Mail. Sonst geben Sie ihn auf dem Server aus:

   ```bash
   docker exec $(docker ps --quiet --filter name=srv-captain--kaeso-gateway) kaeso setup-link
   ```

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags in `kaeso.yml`. Öffnen Sie in jeder der Apps `kaeso-gateway`, `kaeso-console` und `kaeso` **Deployment** und rollen Sie per Image-Name mit dem neuen Tag aus, etwa `ghcr.io/logicplanes/kaeso-gateway:{{version}}`, das Gateway zuerst. Das Gateway migriert die Datenbanken beim Start, nur vorwärts ([Upgrades](../upgrades.md)). Zum Zurückrollen rollen Sie den früheren Tag genauso aus; ein Release, dessen Migrationen die frühere Version nicht ausführen kann, bleibt, wie auf jeder Installation.

## Grenzen

- CapRover liest aus einer One-Click-App nur einige Compose-Felder. Health-Checks, entzogene Capabilities, ein schreibgeschütztes Root-Dateisystem und Speichergrenzen erreichen die Apps nicht, sie laufen also mit den Standards der Images.
- nginx wartet standardmäßig 60 Sekunden auf eine Antwort. Streams senden Keepalives, aber eine Anfrage ohne Streaming, die länger braucht, bekommt `504`. Erhöhen Sie `proxy_read_timeout` in der nginx-Konfiguration der Edge-App, wenn Sie mehr brauchen.
- Der Updater von Kaeso läuft hier nicht, die Konsole kann Kaeso also nicht aktualisieren; Sie rollen jedes Release in CapRover aus.

## Worauf das beruht

Die Vorlage folgt der Anleitung von CapRover zu One-Click-Apps, den Compose-Feldern, die sein Quellcode liest, und seiner nginx-Konfiguration, gelesen am 9. November 2026. Sie lässt sich als YAML lesen, und jedes Feld, das sie nutzt, liest CapRover. Sie wurde noch nicht auf CapRover ausgerollt. Tun Sie das zuerst auf einem Wegwerf-Server und prüfen Sie den Einrichtungslink, eine gestreamte Antwort und die Client-Adresse in einem Anfrageprotokoll ([was geprüft wurde](index.md#was-geprüft-wurde)).

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Sichern Sie PostgreSQL und bewahren Sie den Hauptschlüssel außerhalb des Servers auf.
- [Domains und TLS](../domains-and-tls.md): Lange Streams und was ein Proxy vor Kaeso braucht.
