---
title: Fehlerbehebung
description: Häufige Probleme mit einer Installation und wie sie sich lösen lassen.
order: 9
---

Mit dem Log des Gateways beginnen. Es nennt die betroffene Einstellung, Anfrage oder den betroffenen Dienst.

```bash
docker compose logs --tail 100 gateway
```

## Das Gateway startet nicht

Das Gateway prüft beim Start seine Einstellungen und hält mit einer Meldung an, die die Einstellung nennt. `docker compose up -d --wait` bricht dann mit einem Fehler zum Container des Gateways ab, und `docker compose ps` zeigt, dass er neu startet.

- Prüfen, ob `.env` `KAESO_DOMAIN`, `POSTGRES_PASSWORD`, `POSTGRES_SUPERUSER_PASSWORD`, `CLICKHOUSE_PASSWORD` und `VALKEY_PASSWORD` setzt und ob `master-keys` neben `compose.yaml` liegt. Compose selbst startet ohne eine dieser Variablen nicht und nennt die fehlende.
- Eine Meldung zu `KAESO_SMTP_HOST` bedeutet, dass E-Mails kein Ziel haben: den SMTP-Server eintragen oder `KAESO_MAIL_TRANSPORT=log` setzen, um Kaeso ohne ihn auszuprobieren. Siehe [E-Mail](email.md).
- Prüfen, ob jeder Wert in `compose.override.yaml` einer ist, den die [Konfiguration](configuration.md) erlaubt.

Nennt die Meldung eine Hauptschlüssel-ID, die nicht konfiguriert ist, enthält die Datenbank Secrets, die mit einem Schlüssel verschlüsselt sind, der in `master-keys` fehlt. Den Schlüssel wieder hinzufügen; siehe [Backups und Hauptschlüssel](backups.md#hauptschlüssel).

Sagt sie, die Datenbank sei von einer neueren Version von Kaeso migriert worden, startet ein älteres Image gegen eine neuere Datenbank. Die neuere Version starten oder das Backup von vor dem Upgrade wiederherstellen.

Lehnen PostgreSQL oder ClickHouse das Passwort des Gateways ab, wurde das Passwort in `.env` geändert, nachdem die Datenbanken angelegt waren; die Datenbanken behalten das Passwort, mit dem sie gestartet sind. Das alte Passwort wieder in `.env` eintragen.

## Kein Einrichtungslink, oder er ist abgelaufen

Der Link wird nur ausgegeben, solange die Installation keine Benutzer hat, und funktioniert einmal, 24 Stunden lang. Einen neuen ausgeben:

```bash
docker compose exec gateway kaeso setup-link
```

Ist Kaeso bereits eingerichtet, sagt der Befehl das; stattdessen unter der eigenen Domain anmelden.

## Die Seite hat kein Zertifikat

Caddy braucht einen DNS-Eintrag der Domain, der auf den Server zeigt, und offene Ports 80 und 443. `docker compose logs caddy` zeigt die Antwort der Zertifizierungsstelle. Siehe [Domains und TLS](domains-and-tls.md#zertifikate).

## Jemand hat den zweiten Faktor verloren

Ein Instanz-Admin setzt ihn unter **Einstellungen** › **Instanz** › **Benutzer** zurück, im Sheet der Person, mit **Zwei-Faktor zurücksetzen**. Gibt es keinen Instanz-Admin, der sich anmelden kann, ihn vom Server aus zurücksetzen:

```bash
docker compose exec gateway kaeso user reset-2fa person@example.com
```

Der Befehl entfernt den Authenticator und die Wiederherstellungscodes der Person, beendet ihre Sitzungen, benachrichtigt sie per E-Mail und vermerkt das im Audit-Log der Instanz. Bei der nächsten Anmeldung richtet die Person die Zwei-Faktor-Authentifizierung neu ein, wo sie vorgeschrieben ist.

## Anfragen schlagen fehl

Der Fehler kommt im Format, das die App aufgerufen hat, und sagt, was zu tun ist. Die Seite [Fehler](../api/errors.md) listet sie auf. Die häufigsten:

| Status | Warum | Was tun |
| --- | --- | --- |
| 401 | Der Schlüssel fehlt, ist unbekannt, deaktiviert, archiviert oder abgelaufen | Den Schlüssel prüfen, den die App sendet, und seinen Zustand unter **API-Schlüssel** |
| 403 | Das Mitglied des Schlüssels oder der Schlüssel darf das Modell nicht nutzen | Den Modellzugriff des Mitglieds oder des Schlüssels erweitern |
| 404 | Das Modell steht nicht im Katalog des Projekts | Es unter **Modelle** hinzufügen oder ein Modell aufrufen, das dort steht |
| 400 | Für den Anbieter des Modells ist kein Anbieter-Schlüssel eingerichtet | Einen unter **Anbieter** hinzufügen oder dem Projekt einen zuweisen |
| 429 | Ein Ratenlimit oder ein Budget, oder zu viele unbekannte Schlüssel von einer Adresse | Die Meldung nennt das Limit; `Retry-After` abwarten oder das Limit erhöhen |
| 502, 504 | Der Anbieter war nicht erreichbar, hat nicht rechtzeitig geantwortet oder eine Antwort gesendet, die größer als `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` ist | Den Status des Anbieters prüfen; das Anfrageprotokoll zeigt seinen Fehler |
| 503 | Valkey ist nicht erreichbar und ein hartes Budget lässt sich nicht prüfen, oder das Gateway startet neu | [`/readyz`](monitoring.md#health-checks) und `docker compose logs valkey` prüfen |

In den OpenAI-Formaten kommen manche der eigenen 503-Fehler des Gateways als 500 an.

## Streams brechen nach einer Minute ab

Ein Proxy oder Load Balancer zwischen App und Kaeso schließt untätige Verbindungen. Das Gateway schreibt alle 15 Sekunden ein Keepalive, solange ein Stream still ist; `KAESO_STREAMS_KEEPALIVE_INTERVAL` in [`compose.override.yaml`](installation.md#lokale-änderungen) senken, wenn ein Proxy auf dem Weg Verbindungen früher schließt. Bei einem eigenen Proxy das Puffern von Antworten für `/v1/` abschalten. Siehe [lange Streams](domains-and-tls.md#lange-streams).

## Anfrageprotokolle fehlen oder kommen spät

Einträge erscheinen einige Sekunden nach der Anfrage. Erscheinen keine, nimmt ClickHouse möglicherweise keine Schreibvorgänge an: `/readyz` meldet `degraded`, und die Metrik `kaeso_log_buffer_rows` wächst. Zeilen warten im Speicher bis zu `KAESO_LOGS_BUFFER_SIZE`, dann werden die ältesten verworfen. `docker compose logs clickhouse` prüfen.

## Jemand meldet eine Anfrage-ID

Scheitert das Gateway auf eine Weise, die der Aufrufer nicht beheben kann, antwortet die Platform API mit 500, und die Konsole zeigt *Im Gateway ist etwas schiefgelaufen; Anfrage req_… an die Administration melden.* Jede [Logzeile](monitoring.md#logs), die eine Anfrage auslöst, trägt deren ID, daher findet `docker compose logs gateway | grep req_…` die Zeile `internal error` mit ihrer Ursache.

## Kaeso erreicht seine Datenbank nicht

Ist PostgreSQL ausgefallen oder lehnt Verbindungen ab, antwortet die Platform API mit 503, die Konsole zeigt *Kaeso erreicht seine Datenbank gerade nicht; gleich erneut versuchen.*, und [`/readyz`](monitoring.md#health-checks) antwortet mit `not_ready` und `postgres` `down`. `docker compose ps postgres` und `docker compose logs postgres` prüfen: Meist ist die Festplatte voll oder das Passwort hat sich geändert. Sobald die Datenbank zurück ist, funktionieren Anfragen wieder; nichts muss neu starten. Dieselbe 503 gibt die Platform API, wenn Valkey oder ClickHouse nicht erreichbar sind und eine Anfrage sie braucht; nur Anfrageprotokolle und Nutzung melden stattdessen, dass sie gerade nicht verfügbar sind.

## E-Mails kommen nicht an

Einen Test über **Einstellungen** › **Instanz** › **Einstellungen** › **Test-E-Mail senden** senden und im Log des Gateways die Antwort des SMTP-Servers prüfen. `KAESO_SMTP_HOST`, den Port, `KAESO_SMTP_TLS` und die Zugangsdaten in `.env` prüfen und ob der SMTP-Server von der Adresse in `KAESO_SMTP_FROM` senden darf. Ist `KAESO_MAIL_TRANSPORT` `log`, werden E-Mails nur protokolliert. Siehe [E-Mail](email.md).
