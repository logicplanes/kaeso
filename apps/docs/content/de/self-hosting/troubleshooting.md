---
title: Fehlerbehebung
description: Häufige Probleme mit einer Installation und ihre Lösung.
order: 11
---

Mit dem Log des Gateways beginnen; es nennt die Einstellung, die Anfrage oder den betroffenen Dienst:

```bash
docker compose logs --tail 100 gateway
```

## Das Gateway startet nicht

Fehlt eine Einstellung oder ist sie ungültig, hält das Gateway mit einer Meldung an, die sie nennt; `docker compose up -d --wait` scheitert dann am Container des Gateways, und `docker compose ps` zeigt ihn im Neustart.

| Die Meldung sagt | Lösung |
| --- | --- |
| Eine Variable der `.env` fehlt | Compose selbst startet nicht und nennt sie: `KAESO_DOMAIN`, `POSTGRES_PASSWORD`, `POSTGRES_SUPERUSER_PASSWORD`, `CLICKHOUSE_PASSWORD` und `VALKEY_PASSWORD` setzen und prüfen, ob `master-keys` neben der `compose.yaml` liegt |
| `KAESO_SMTP_HOST` ist erforderlich | Den SMTP-Server eintragen oder `KAESO_MAIL_TRANSPORT=log` setzen, um Kaeso ohne ihn auszuprobieren; siehe [E-Mail](email.md) |
| Eine andere Einstellung ist ungültig | Sie in der `compose.override.yaml` korrigieren; [Konfiguration](configuration.md) nennt die erlaubten Werte |
| Eine Hauptschlüssel-ID ist nicht konfiguriert | Die Datenbank enthält Secrets, die mit einem Schlüssel verschlüsselt sind, der in `master-keys` fehlt; ihn wieder eintragen, siehe [Hauptschlüssel](backups.md#hauptschlüssel) |
| Die Datenbank wurde von einer neueren Kaeso-Version migriert | Ein älteres Image startet gegen eine neuere Datenbank; die neuere Version starten oder ein [Downgrade](upgrades.md#downgrade) durchführen |
| PostgreSQL oder ClickHouse lehnt das Passwort ab | Das Passwort in der `.env` wurde geändert, nachdem die Datenbanken angelegt wurden, und diese behalten ihr erstes; das alte Passwort wieder eintragen |

## Kein Einrichtungslink, oder er ist abgelaufen

Den Einrichtungslink gibt es nur, solange die Installation keine Benutzer hat; er funktioniert einmal, 24 Stunden lang. Einen neuen ausgeben:

```bash
docker compose exec gateway kaeso setup-link
```

Ist Kaeso schon eingerichtet, sagt der Befehl das; dann unter der eigenen Domain anmelden.

## Ausgesperrt aus einer Installation ohne Passwörter

Auf einer Installation, die sich nur per Single Sign-on anmeldet, lässt ein defekter Identitätsanbieter oder eine Einrichtungssitzung, die endete, bevor Single Sign-on funktionierte, keinen Weg hinein. Passwörter nicht wieder einschalten, sondern mit `kaeso user sign-in-link` einen einmaligen Anmeldelink für einen Instanz-Admin ausgeben, wie [Wieder hineinkommen](sign-in.md#wieder-hineinkommen) beschreibt.

## Die Seite hat kein Zertifikat

Caddy braucht einen DNS-Eintrag der Domain, der auf den Server zeigt, und offene Ports 80 und 443. `docker compose logs caddy` zeigt die Antwort der Zertifizierungsstelle; siehe [Zertifikate](domains-and-tls.md#zertifikat-beziehen).

## Jemand hat den zweiten Faktor verloren

Ein Instanz-Admin setzt ihn unter **Einstellungen** › **Instanz** › **Benutzer** im Sheet der Person mit **Zwei-Faktor zurücksetzen** zurück. Gibt es keinen Instanz-Admin, der sich anmelden kann, ihn auf dem Server zurücksetzen:

```bash
docker compose exec gateway kaeso user reset-2fa person@example.com
```

In beiden Fällen werden Authenticator und Wiederherstellungscodes der Person entfernt, ihre Sitzungen beendet, sie erhält eine E-Mail, und das Audit-Log der Instanz verzeichnet den Vorgang. Wo Zwei-Faktor-Authentifizierung verlangt wird, richtet sie sie bei der nächsten Anmeldung neu ein.

## Anfragen schlagen fehl

Der Fehler kommt im Format, das die App aufgerufen hat, und sagt, was zu tun ist; [Fehler](../api/errors.md) listet alle auf. Die häufigsten:

| Status | Warum | Was tun |
| --- | --- | --- |
| 400 | Für den Anbieter des Modells ist kein Anbieter-Schlüssel eingerichtet | Einen unter **Anbieter** anlegen oder dem Projekt zuweisen |
| 401 | Der Schlüssel fehlt, ist unbekannt, deaktiviert, archiviert oder abgelaufen | Den Schlüssel prüfen, den die App sendet, und seinen Zustand unter **API-Schlüssel** |
| 403 | Der Schlüssel oder sein Mitglied darf das Modell nicht verwenden | Den Modellzugriff des Mitglieds oder des Schlüssels erweitern |
| 404 | Das Modell steht nicht im Katalog des Projekts | Es unter **Modelle** hinzufügen oder ein vorhandenes Modell aufrufen |
| 429 | Ein Ratenlimit oder Budget, oder zu viele unbekannte Schlüssel von einer Adresse | Die Meldung nennt das Limit; `Retry-After` abwarten oder das Limit erhöhen |
| 502, 504 | Der Anbieter war nicht erreichbar, hat nicht rechtzeitig geantwortet oder eine Antwort größer als `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` gesendet | Den Status des Anbieters prüfen; das Anfrageprotokoll zeigt seinen Fehler |
| 503 | Valkey ist ausgefallen und ein hartes Budget lässt sich nicht prüfen, oder das Gateway startet neu | [`/readyz`](monitoring.md#health-checks) und `docker compose logs valkey` prüfen |

In den OpenAI-Formaten kommen manche eigene 503-Fehler des Gateways als 500 an.

## Streams brechen nach einer Minute ab

Ein Proxy oder Load Balancer zwischen App und Kaeso schließt Verbindungen im Leerlauf. Das Gateway schreibt alle 15 Sekunden ein Keepalive, solange ein Stream schweigt; schließt ein Proxy auf dem Weg Verbindungen früher, `KAESO_STREAMS_KEEPALIVE_INTERVAL` in der [`compose.override.yaml`](installation.md#dienste-ändern) senken. Hinter einem eigenen Proxy außerdem das Puffern von Antworten für `/v1/` abschalten. Siehe [Lange Streams](domains-and-tls.md#lange-streams).

## Anfrageprotokolle fehlen oder kommen spät

Einträge erscheinen einige Sekunden nach der Anfrage. Erscheinen keine, nimmt ClickHouse möglicherweise keine Schreibvorgänge an: `/readyz` meldet `degraded`, und `kaeso_log_buffer_rows` wächst. Zeilen warten im Speicher bis zu `KAESO_LOGS_BUFFER_SIZE`, dann werden die ältesten verworfen. `docker compose logs clickhouse` prüfen und im Log des Gateways nach `ClickHouse migrations failed` suchen.

## Jemand meldet eine Anfrage-ID

Scheitert das Gateway auf eine Weise, die der Aufrufer nicht beheben kann, antwortet die Platform API mit 500, und die Konsole zeigt *Im Gateway ist etwas schiefgelaufen; Anfrage req_… an die Administration melden.* Jede [Logzeile](monitoring.md#logs) einer Anfrage trägt deren ID, daher findet `docker compose logs gateway | grep req_…` die Zeile `internal error` mit ihrer Ursache.

## Kaeso erreicht seine Datenbank nicht

Ist PostgreSQL ausgefallen oder lehnt Verbindungen ab, antworten Konsole und Platform API mit 503 und *Kaeso erreicht seine Datenbank gerade nicht; gleich erneut versuchen.*, und [`/readyz`](monitoring.md#health-checks) antwortet mit `not_ready` und `postgres` `down`. `docker compose ps postgres` und `docker compose logs postgres` prüfen; meist ist die Festplatte voll oder das Passwort hat sich geändert. Sobald die Datenbank zurück ist, funktionieren Anfragen wieder, ohne Neustart.

Dieselbe 503 gibt die Platform API, wenn eine Anfrage Valkey oder ClickHouse braucht und sie nicht erreicht; nur Anfrageprotokolle und Nutzung melden stattdessen, dass sie gerade nicht verfügbar sind.

## E-Mails kommen nicht an

Mit **Test-E-Mail senden** unter **Einstellungen** › **Instanz** › **Einstellungen** einen Test senden und im Log des Gateways die Antwort des SMTP-Servers nachsehen. `KAESO_SMTP_HOST`, den Port, `KAESO_SMTP_TLS` und die Zugangsdaten in der `.env` prüfen und ob der SMTP-Server von der Adresse in `KAESO_SMTP_FROM` senden darf. Mit `KAESO_MAIL_TRANSPORT=log` werden E-Mails nur protokolliert. Siehe [E-Mail](email.md).
