---
title: Fehlerbehebung
description: Finden Sie die Meldung oder das Symptom, das Sie auf einer Installation sehen, und beheben Sie es.
order: 11
---

Suchen Sie die Meldung oder das Symptom, das Sie sehen, und folgen Sie der Lösung. Die meisten Probleme zeigen sich im Protokoll des Gateways, das die beteiligte Einstellung, Anfrage oder den Dienst nennt:

```bash
docker compose logs --tail 100 gateway
```

## Das Gateway startet immer wieder neu

Das Gateway hält an, wenn eine Einstellung fehlt oder ungültig ist, und sein Container startet neu. `docker compose up -d --wait` scheitert dann am Gateway.

1. Führen Sie `docker compose logs --tail 20 gateway` aus und suchen Sie die Zeile, die mit `configuration:` beginnt. Sie nennt die Einstellung, nie ihren Wert.
2. Korrigieren Sie die Einstellung in der `compose.override.yaml` oder in der `.env`, wenn sie zu den [Einstellungen in der .env](configuration.md#einstellungen-in-der-env) gehört.
3. Führen Sie `docker compose up -d --wait` erneut aus.

## Compose verlangt eine Variable in der .env

Eine erforderliche Variable der `.env` fehlt, deshalb verweigert Compose den Start und nennt sie, etwa `set KAESO_DOMAIN in .env`.

1. Setzen Sie `KAESO_DOMAIN`, `POSTGRES_PASSWORD`, `POSTGRES_SUPERUSER_PASSWORD`, `CLICKHOUSE_PASSWORD` und `VALKEY_PASSWORD` in der `.env`.
2. Prüfen Sie, dass `master-keys` neben der `compose.yaml` liegt.
3. Führen Sie `docker compose up -d --wait` erneut aus.

## KAESO_SMTP_HOST is required

Das Gateway hat keinen SMTP-Server, über den es E-Mails senden kann.

1. Tragen Sie Ihren SMTP-Server in die `.env` ein, wie [SMTP einrichten](email.md#smtp-einrichten) zeigt.
2. Oder setzen Sie `KAESO_MAIL_TRANSPORT=log` in der `.env`, um Kaeso ohne Mailserver auszuprobieren.
3. Führen Sie `docker compose up -d --wait` aus.

## Stored secrets are encrypted with master keys that are not configured

Die Datenbank enthält Secrets, die mit einem Hauptschlüssel verschlüsselt sind, der in `master-keys` fehlt. Die Meldung nennt die Schlüssel-ID.

1. Suchen Sie den Schlüssel mit dieser ID in Ihrer Kopie der Hauptschlüssel.
2. Fügen Sie ihn wieder in `master-keys` ein, nach dem aktiven Schlüssel ([Hauptschlüssel rotieren](backups.md#hauptschlüssel-rotieren)).
3. Erstellen Sie das Gateway mit `docker compose up -d --wait --force-recreate gateway` neu.

## The database was migrated by a newer version of Kaeso

Ein älteres Gateway startet mit einer Datenbank, die ein neueres Release migriert hat, und Migrationen lassen sich nicht rückgängig machen.

1. Starten Sie wieder die `compose.yaml` des neueren Releases.
2. Um zum älteren Release zurückzukehren, stellen Sie ein Backup von vor dem Upgrade wieder her ([Downgrade](upgrades.md#downgrade)).

## PostgreSQL lehnt das Passwort des Gateways ab

`POSTGRES_PASSWORD` in der `.env` hat sich geändert, nachdem das Volume von PostgreSQL angelegt wurde. PostgreSQL behält das Passwort von damals.

1. Setzen Sie das alte Passwort wieder in die `.env`.
2. Führen Sie `docker compose up -d --wait` aus.

## Die Website hat kein Zertifikat

Caddy konnte kein Zertifikat für `KAESO_DOMAIN` beziehen, meist wegen DNS oder einer Firewall.

1. Führen Sie `docker compose logs caddy` aus und lesen Sie die Antwort der Zertifizierungsstelle.
2. Prüfen Sie, dass der DNS-Eintrag der Domain auf den Server zeigt und die Ports 80 und 443 zum Internet offen sind.
3. Führen Sie `docker compose restart caddy` aus.

## Der Einrichtungslink fehlt oder ist abgelaufen

Der Einrichtungslink funktioniert einmal, 24 Stunden lang, und nur, solange die Installation keine Benutzer hat. Geben Sie einen neuen aus:

```bash
docker compose exec gateway kaeso setup-link
```

Ist Kaeso schon eingerichtet, sagt der Befehl das. Die Einrichtungsseite meldet dasselbe, mit einem Link zur Anmeldung. Melden Sie sich stattdessen unter Ihrer Domain an.

## E-Mails kommen nicht an

Das Gateway erreicht den SMTP-Server nicht, oder der Server lehnt die E-Mail ab.

1. Wählen Sie **Test-E-Mail senden** unter **Instanz** › **System**.
2. Lesen Sie die Antwort des SMTP-Servers im Protokoll des Gateways.
3. Prüfen Sie `KAESO_SMTP_HOST`, `KAESO_SMTP_PORT`, `KAESO_SMTP_TLS` und die Zugangsdaten in der `.env`, und ob der Server von `KAESO_SMTP_FROM` senden darf.

Mit `KAESO_MAIL_TRANSPORT=log` protokolliert das Gateway E-Mails nur und sendet sie nie ([E-Mail](email.md)).

## Anfragen scheitern

Der Fehler kommt im Format, das die App aufgerufen hat, und seine Meldung sagt, was zu tun ist. [Fehler](../api/errors.md) listet jeden Fehler; das sind die häufigsten:

| Status | Ursache | Lösung |
| --- | --- | --- |
| 400 | Für den Anbieter des Modells ist kein Anbieter-Schlüssel eingerichtet | Fügen Sie unter **Anbieter** einen hinzu oder weisen Sie dem Projekt einen zu |
| 401 | Der Schlüssel fehlt, ist unbekannt, deaktiviert, archiviert oder abgelaufen | Prüfen Sie den Schlüssel, den die App sendet, und seinen Zustand unter **API-Schlüssel** |
| 404 | Das Modell steht nicht im Katalog des Projekts, oder der Schlüssel oder sein Mitglied darf es nicht verwenden | Fügen Sie es unter **Modelle** hinzu oder rufen Sie ein Modell auf, das dort steht; steht es dort, prüfen Sie den Modellzugriff des Schlüssels und seines Mitglieds |
| 429 | Ein Ratenlimit oder ein Budget, oder zu viele unbekannte Schlüssel von einer Adresse | Warten Sie `Retry-After` ab oder erhöhen Sie das Limit, das die Meldung nennt |
| 502, 504 | Der Anbieter war nicht erreichbar, hat nicht rechtzeitig geantwortet oder eine Antwort größer als `KAESO_BODY_LIMITS_PROVIDER_RESPONSE` gesendet | Prüfen Sie den Status des Anbieters; das Anfrageprotokoll zeigt seinen Fehler |
| 503 | Valkey ist nicht erreichbar und ein hartes Budget lässt sich nicht prüfen, oder das Gateway startet neu | Prüfen Sie [`/readyz`](monitoring.md#health-checks) und `docker compose logs valkey` |

In den OpenAI-Formaten kommen manche 503-Fehler des Gateways als 500 an.

## Streams brechen nach einer Minute ab

Ein Proxy oder Load Balancer zwischen App und Kaeso schließt Verbindungen, die zu lange ruhen.

1. Setzen Sie `KAESO_STREAMS_KEEPALIVE_INTERVAL` in der `compose.override.yaml` unter das Leerlauf-Zeitlimit des Proxys. Der Standard ist 15 Sekunden.
2. Hinter Ihrem eigenen Proxy schalten Sie die Pufferung von Antworten für `/v1/` ab.

[Lange Streams](domains-and-tls.md#lange-streams) erklärt die Keepalives.

## Anfrageprotokolle fehlen oder kommen spät

Einträge erscheinen wenige Sekunden nach der Anfrage. Erscheinen keine, nimmt ClickHouse keine Schreibvorgänge an: `/readyz` meldet `degraded`, und `kaeso_log_buffer_rows` wächst. Zeigt die Konsole gar keine **Anfrageprotokolle** und meldet `/readyz` `clickhouse` als `off`, speichert die Installation keine Anfrageprotokolle ([festlegen, wie Anfrageprotokolle gespeichert werden](installation.md#festlegen-wie-anfrageprotokolle-gespeichert-werden)).

1. Führen Sie `docker compose logs clickhouse` aus und beheben Sie, was dort gemeldet wird.
2. Suchen Sie im Protokoll des Gateways nach `ClickHouse migrations failed`.

Zeilen warten im Speicher des Gateways bis zu `KAESO_LOGS_BUFFER_SIZE`, danach verwirft Kaeso die ältesten.

## Kaeso erreicht seine Datenbank nicht

Konsole und Platform API antworten 503 mit „Kaeso erreicht seine Datenbank gerade nicht; gleich erneut versuchen.“ PostgreSQL ist nicht erreichbar oder lehnt Verbindungen ab, und [`/readyz`](monitoring.md#health-checks) antwortet `not_ready`.

1. Führen Sie `docker compose ps postgres` und `docker compose logs postgres` aus.
2. Beheben Sie die Ursache. Meist ist es eine volle Festplatte oder ein geändertes Passwort.

Anfragen funktionieren wieder, sobald die Datenbank zurück ist, ohne Neustart. Dieselbe 503 gibt die Platform API, wenn eine Anfrage Valkey oder ClickHouse braucht und sie nicht erreicht.

## Jemand hat seinen zweiten Faktor verloren

Ein Instanz-Admin setzt ihn unter **Instanz** › **Benutzer** zurück, im Sheet der Person, mit **Zwei-Faktor zurücksetzen**. Ohne einen Instanz-Admin, der sich anmelden kann, setzen Sie ihn auf dem Server zurück:

```bash
docker compose exec gateway kaeso user reset-2fa owner@example.com
```

In beiden Fällen entfernt Kaeso Authenticator und Wiederherstellungscodes, beendet die Sitzungen der Person und sendet ihr eine E-Mail. Wo Zwei-Faktor-Authentifizierung verlangt ist, richtet sie sie bei der nächsten Anmeldung neu ein.

## Zwei Konten teilen sich eine Adresse

Kaeso behandelt E-Mail-Adressen, die sich nur in der Schreibweise unterscheiden, als eine Adresse. Dazu gehören Groß- und Kleinschreibung, ein Akzent als Teil seines Buchstabens oder dahinter getippt sowie Buchstaben in voller Breite. Konten, die angelegt wurden, bevor Kaeso Adressen so verglich, können sich eine Adresse teilen.

Kaeso meldet dann keines der beiden über diese Adresse an, schickt keinem einen Link und protokolliert `accounts share one canonical email address; refusing to look either up by it` mit beiden Benutzer-IDs. Ein Instanz-Admin findet beide unter **Instanz** › **Benutzer** und löscht das Konto, das es nicht geben sollte. Das andere meldet sich danach wieder an wie zuvor.

## Niemand kann sich über Single Sign-on anmelden

Auf einer Installation ohne Passwörter lässt ein defekter Identitätsanbieter keinen Weg hinein. Schalten Sie Passwörter nicht wieder ein: Geben Sie einen einmaligen Anmeldelink für einen Instanz-Admin aus, wie [Wieder hineinkommen](sign-in.md#wieder-hineinkommen) zeigt.

## Jemand meldet eine Anfrage-ID

Das Gateway ist auf eine Weise gescheitert, die der Aufrufer nicht beheben kann, und sein Log enthält die Ursache. Die Konsole zeigte „Im Gateway ist etwas schiefgelaufen; Anfrage req_… an die Administration melden.“

1. Suchen Sie die ID im Protokoll: `docker compose logs gateway | grep req_…`.
2. Lesen Sie die Zeile `internal error` mit ihrer Ursache.
