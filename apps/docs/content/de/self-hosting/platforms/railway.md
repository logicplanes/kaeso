---
title: Railway
description: Rollen Sie Kaeso mit dem Button Deploy on Railway oder aus der Infrastrukturdatei auf Railway aus, mit Railway Postgres und Redis.
order: 2
---

Der Button **Deploy on Railway** rollt Kaesos [Railway-Vorlage](https://railway.com/deploy/cmVV60) aus. `.railway/railway.ts` im Repository beschreibt dasselbe Projekt als [Infrastructure as Code](https://docs.railway.com/infrastructure-as-code), für die Railway-CLI. Beide betreiben einen öffentlichen Edge, ein privates Gateway und eine private Konsole, einen Postgres-Dienst und einen Redis-Dienst, alle auf ein Release festgelegt. Railways ältere `railway.json` und `railway.toml` funktionieren ab dem 1. Dezember 2026 nicht mehr, das Repository hat also keine.

## Was Railway betreibt

| Dienst | Image | Erreichbar für |
| --- | --- | --- |
| `kaeso-edge` | Das Konsolen-Image, mit `kaeso-edge` | Alle, über seine Railway-Domain oder Ihre eigene |
| `kaeso-gateway` | Das Gateway-Image | Den Edge, über Railways privates Netz |
| `kaeso-console` | Das Konsolen-Image | Den Edge, über Railways privates Netz |
| `kaeso-postgres` | Railway Postgres | Das Gateway |
| `kaeso-valkey` | Railway Redis | Das Gateway |

Railways privates Netz ist reines IPv6, das Gateway lauscht also auf `[::]:8080` und vertraut dem Edge aus dem Unique-Local-Adressbereich. Railways Health-Checks rufen den Port in `PORT` auf, daher setzt jeder Dienst ihn auf den Port, auf dem er lauscht. Jedes Image hat automatische Updates aus, die Dienste bleiben also auf dem Release, mit dem sie ausgerollt wurden.

## Mit dem Button ausrollen

1. Öffnen Sie [Deploy on Railway](https://railway.com/new/template/cmVV60?utm_medium=integration&utm_source=button&utm_campaign=kaeso) und melden Sie sich bei Railway an.
2. Tragen Sie unter `kaeso-gateway` diese Variablen ein:

   | Variable | Was einzutragen ist |
   | --- | --- |
   | `KAESO_SETUP_EMAIL` | Ihre Adresse; das Gateway sendet den Einrichtungslink dorthin |
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_FROM` | Ihr SMTP-Server und der Absender von Kaesos E-Mails ([E-Mail](../email.md)); ohne SMTP-Server startet das Gateway nicht |
   | `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD` | Benutzername und Passwort des Servers, falls er sie braucht |
   | `KAESO_SMTP_PORT` | `587`, außer Ihr Server nutzt einen anderen Port |
   | `KAESO_CLICKHOUSE_URL` | Eine ClickHouse-HTTP-URL mit Benutzer und Passwort für Anfrageprotokolle, oder leer, um sie aus zu lassen |

   Den Rest füllt die Vorlage: `KAESO_MASTER_KEYS` mit einem neuen Hauptschlüssel aus 32 zufälligen Bytes von Railways Funktion `secret()` und `KAESO_PUBLIC_URL` mit der Railway-Domain des Edge.

3. Rollen Sie aus und warten Sie, bis die fünf Dienste online sind. Das Gateway sendet den einmaligen Einrichtungslink an `KAESO_SETUP_EMAIL`; öffnen Sie ihn, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)).
4. Kopieren Sie `KAESO_MASTER_KEYS` aus den Variablen von `kaeso-gateway` in einen Passwort-Manager. Kaeso verschlüsselt Anbieterschlüssel damit, ohne ihn ist ein Datenbank-Backup also nicht lesbar ([Backups und Hauptschlüssel](../backups.md)).
5. Für Ihre eigene Domain fügen Sie sie unter **Settings** zu `kaeso-edge` hinzu, setzen `KAESO_PUBLIC_URL` von `kaeso-gateway` auf `https://` und diese Domain und rollen das Gateway neu aus.

> [!NOTE]
> Das Gateway-Image hat keine Shell, `railway ssh` kann `kaeso setup-link` darin also nicht ausführen. Kommt die Einrichtungs-E-Mail nicht an, nennen die Protokolle des Gateways den SMTP-Fehler. Korrigieren Sie die SMTP-Variablen und rollen Sie `kaeso-gateway` neu aus: Solange kein Benutzer existiert, sendet es bei jedem Start einen neuen Link.

## Mit der CLI ausrollen

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
   | `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_USER`, `KAESO_SMTP_PASSWORD`, `KAESO_SMTP_FROM` | Ihr SMTP-Server ([E-Mail](../email.md)); ohne ihn startet das Gateway nicht |
   | `KAESO_CLICKHOUSE_URL` | Eine ClickHouse-HTTP-URL mit Benutzer und Passwort für Anfrageprotokolle, oder leer, um sie aus zu lassen |

4. Legen Sie die `.railway/railway.ts` des Releases in einen eigenen Ordner, von `https://github.com/logicplanes/kaeso/blob/v{{version}}/.railway/railway.ts`, installieren Sie dort Railways TypeScript-Paket und wenden Sie die Datei an:

   ```bash
   npm install railway
   railway config apply
   ```

5. Geben Sie `kaeso-edge` eine Domain: Erzeugen Sie unter seinen **Settings** eine Railway-Domain oder fügen Sie Ihre eigene hinzu, für Port 8080.
6. Setzen Sie `KAESO_PUBLIC_URL` auf `https://` und diese Domain und rollen Sie `kaeso-gateway` neu aus. Das Gateway sendet den einmaligen Einrichtungslink an `KAESO_SETUP_EMAIL`; öffnen Sie ihn, um den ersten Benutzer anzulegen ([der erste Start](../how-it-works.md#der-erste-start)).

## Aktualisieren und zurückrollen

Ein Release ändert die Image-Tags. Bei einem Projekt aus dem Button ändern Sie den Tag im Image von `kaeso-gateway`, `kaeso-console` und `kaeso-edge` in den **Settings** jedes Dienstes, das Gateway zuerst. Bei einem Projekt aus der Datei wenden Sie die `railway.ts` des neuen Releases genauso an wie zuvor; Railway rollt die geänderten Dienste neu aus. In beiden Fällen migriert das Gateway die Datenbanken beim Start, nur vorwärts ([Upgrades](../upgrades.md)). Zum Zurückrollen rollen Sie ein früheres Deployment eines Dienstes aus seiner Liste **Deployments** neu aus.

## Worauf das beruht

Die Datei folgt Railways TypeScript-Referenz, gelesen am 6. November 2026: `defineRailway`, `project`, `service`, `image`, `postgres`, `redis`, Dienstumgebungen aus `db.env` und `cache.env`, geteilte Variablen über `ctx.shared` und `${{service.RAILWAY_PRIVATE_DOMAIN}}` für die privaten Adressen. Railway kennzeichnet die Funktion als experimentell, ihre API kann sich also noch ändern.

Am 9. November 2026 wurde die Datei auf ein leeres Railway-Projekt angewendet und die Vorlage in ein weiteres ausgerollt, beide mit dem achten Release Candidate. In beiden kamen die fünf Dienste online, der Edge lieferte Konsole und APIs auf der Railway-Domain aus, und der erste Benutzer wurde eingerichtet und angemeldet. Der erste Lauf ergab, dass Gateway und Konsole ohne `PORT` an Railways Health-Checks scheiterten; Datei und Vorlage setzen ihn jetzt. Der Test nutzte einen Platzhalter-SMTP-Server, der Einrichtungslink kam also von `kaeso setup-link` über einen SSH-Tunnel statt per E-Mail. E-Mail-Zustellung, eine gestreamte Antwort und ein Update wurden auf Railway nicht getestet.

## Nächste Schritte

- [Konfiguration](../configuration.md): Jede Einstellung, die das Gateway liest, und was sie bewirkt.
- [Backups und Hauptschlüssel](../backups.md): Sichern Sie Postgres auf Railway und bewahren Sie den Hauptschlüssel außerhalb auf.
- [Zustand und Metriken](../monitoring.md): Die Health-Checks, die Edge und Gateway beantworten.
