---
title: Platform API
description: Eine Installation über HTTP verwalten, so wie die Konsole es tut.
order: 3
---

Die Platform API verwaltet Organisationen, Projekte, Mitglieder, Schlüssel, Anbieter-Schlüssel, Modelle, Limits, Nutzung und Protokolle. Sie liegt unter `/platform/v1` auf derselben Domain wie die Modell-API. Die Konsole nutzt sie und nichts anderes; alles, was die Konsole tut, kann also auch ein Skript.

Die vollständige Referenz ist das OpenAPI-Dokument, [`openapi.json`](https://github.com/logicplanes/kaeso/blob/dev/openapi.json) im Wurzelverzeichnis des Repositorys, aus dem Code des Gateways erzeugt; `kaeso openapi` gibt das Dokument der laufenden Version aus, zum Beispiel mit `docker compose exec gateway kaeso openapi`. Jede Operation darin nennt das eine Recht, das sie prüft.

## Konventionen

| Thema | Regel |
| --- | --- |
| Form | REST mit JSON; Ressourcennamen im Plural in Kebab-Case (`service-accounts`), Felder in `snake_case`; Bodys werden mit `content-type: application/json` gesendet, und ein Feld, das der Endpunkt nicht kennt, wird abgelehnt |
| Zeiten | RFC 3339, in UTC |
| Geld | US-Dollar als Dezimal-Strings, etwa `"0.0001"`; Preise und Beträge werden als Dezimal-Strings oder ganze Zahlen gesendet, und ein JSON-Bruch wie `0.5` wird mit `400` abgelehnt; der `amount` eines Limits beträgt höchstens 10^12 Anfragen oder Token und bei einem Ausgabenbudget 1.000.000.000 USD |
| IDs | Ein Präfix und ein zeitlich geordneter Teil: `org_…`, `proj_…`, `usr_…`, `key_…`, `opk_…`, `svc_…`, `cred_…`, `model_…`, `alias_…`, `lim_…`, `role_…`, `req_…`, `evt_…` und weitere |
| Listen | `limit` (1 bis 100, Standard 20) und `after`, der `next_cursor` der vorigen Seite; die Antwort ist `{"data": [...], "has_more": true, "next_cursor": "…"}`, mit `next_cursor` `null` auf der letzten Seite; sortiert nach ID, also nach Erstellungszeit, Anfrageprotokolle und Audit-Logs neueste zuerst; Filter sind Query-Parameter |
| Erstellen | `POST` auf die Sammlung, beantwortet mit `201` und der Ressource |
| Aktualisieren | `PATCH` mit den zu ändernden Feldern |
| Revisionen | Katalogmodelle und Limits tragen eine `revision`, die mit jeder Änderung steigt; Lesen, Erstellen und Ändern antworten mit `ETag: "<revision>"`. `PATCH` und `DELETE` nehmen ein optionales `If-Match` mit diesem Tag und antworten mit `412 conflict`, wenn sich der Eintrag inzwischen geändert hat, wie es das OpenAPI-Dokument für diese vier Operationen angibt; ohne `If-Match` gilt die Änderung für die aktuelle Revision |
| Aktionen | `POST …/archive`, `…/disable`, `…/enable`, `…/replace`, `…/restore`; Archivieren ist kein Löschen |
| Löschen | `DELETE`, nur wo wirklich etwas gelöscht wird, etwa bei Anbieter-Schlüsseln, Aliassen, Katalogmodellen, Limits, eigenen Rollen, Sitzungen, Einladungen, dem Konto |
| Rechte | Jede Operation auf `orgs/{org}/*` prüft ein Recht auf der Organisation oder dem Projekt in ihrem Pfad, genannt in ihrer Beschreibung; `auth/*`, `me/*` und `instance/*` folgen eigenen Regeln: öffentlich, der angemeldete Benutzer, Instanz-Admins |
| Kürzliche Anmeldung | Mit einer Sitzung erfordern sensible Aktionen eine Anmeldung in den letzten 15 Minuten: eine Organisation löschen oder wiederherstellen, Plattformzugriff für Schlüssel, Anbieter-Schlüssel hinzufügen und ändern, Plattform-Schlüssel erstellen, Einstellungen zu Single Sign-on und Zwei-Faktor, Protokolle löschen und weitere; sonst lautet die Antwort `403 recent_sign_in_required` |
| Zur Löschung vorgemerkte Organisationen | Schreibgeschützt, bis sie wiederhergestellt oder gelöscht sind: Lesen funktioniert, soweit die Rechte des Aufrufers es erlauben, und jede Änderung in der Organisation, auch sie zu verlassen und eine Einladung in sie anzunehmen, ergibt `409 org_scheduled_for_deletion`, außer `POST orgs/{org}/restore` und `instance/orgs/{org}/*` der Instanz-Admins; ihre Schlüssel authentifizieren gar nicht |
| Audit | Jede Änderung wird in derselben Transaktion in das Audit-Log ihrer Ebene geschrieben |
| Anfrage-IDs | Jede Antwort trägt `kaeso-request-id` |

`auth/session` zeigt eine vorgemerkte Organisation in jeder Mitgliedschaft: ihren `org_state`, `org_deletion_due_at` und Rechte, reduziert auf Sehen, Exportieren und `org.delete`. Es enthält auch die Richtlinie `org_deletion` der Instanz (`allowed`, `grace_days`).

## Authentifizierung

| Methode | Wie | Für |
| --- | --- | --- |
| Sitzung | Das HttpOnly-Cookie `__Host-kaeso_session`, `kaeso_session` bei einer `http://`-Installation unter einer Loopback-Adresse wie localhost, gesetzt von den Anmelde-Endpunkten unter `auth/*`; Änderungen müssen zusätzlich `Origin` mit dem Origin der öffentlichen URL der Installation senden | Die Konsole und Personen mit curl |
| API-Schlüssel mit Plattformzugriff | `Authorization: Bearer sk-kso-…`; handelt mit den Rechten seines Mitglieds oder Dienstkontos, nur im eigenen Projekt | Skripte und CI für ein Projekt |
| Plattform-Schlüssel | `Authorization: Bearer sk-kso-org-…`; handelt mit den Rechten der Rolle Admin in seiner Organisation und jedem Projekt darin und ruft nie Modelle auf | Automatisierung für eine ganze Organisation |

Ein Projektschlüssel hat standardmäßig Inferenzzugriff. Plattformzugriff wird beim Erstellen des Schlüssels gewählt, mit `"access": "platform"` oder `"both"`, und erfordert das Recht `keys.platform_access.grant`. Plattform-Schlüssel werden nur mit einer Sitzung erstellt. Siehe [API-Schlüssel](../console/api-keys.md).

Schlüssel sind für `orgs/{org}/*`: `me/*`, `instance/*` und die angemeldeten Routen von `auth/*` lehnen sie mit `403` ab, ebenso Organisationsrouten bei einem Projektschlüssel. Eine Organisation, die Single Sign-on verlangt, lehnt andere Sitzungen mit `403` ab, und eine, die Zwei-Faktor-Authentifizierung verlangt, lehnt Passwortsitzungen ohne zweiten Faktor ab.

Zu viele Anfragen mit unbekannten API-Schlüsseln von einer Adresse, auf Modell-API und Platform API zusammen, werden mit `429 rate_limited` und `Retry-After` abgelehnt, bis das Zeitfenster endet. [Fehlgeschlagene API-Schlüssel](../self-hosting/configuration.md#fehlgeschlagene-api-schlüssel) nennt die Einstellungen.

### Websiteübergreifende Anfragen

Jede Anfrage außer `GET`, `HEAD` und `OPTIONS`, die das Sitzungs-Cookie trägt, und jede solche Anfrage an einen öffentlichen `auth/*`-Endpunkt muss `Origin` mit dem Origin der öffentlichen URL der Installation senden, etwa `Origin: https://kaeso.example.com`. Browser senden es von selbst. Ohne ihn oder mit einem anderen Origin lautet die Antwort `403 permission_denied`, und ihre Meldung nennt den zu sendenden Header. Die Regel hindert andere Websites daran, mit der Sitzung einer Person zu handeln oder einen Browser in ein eigenes Konto anzumelden; `Referer` zählt nicht an seiner Stelle.

- Anfragen an `orgs/{org}/*` mit einem API-Schlüssel brauchen kein `Origin`.
- Lesende Anfragen brauchen es nie, darunter `GET auth/methods` und der Startlink `GET auth/oidc/start`.
- Ein Server, der `POST auth/methods`, `POST auth/password` oder `POST auth/oidc/start` aufruft, sendet `Origin` mit dem Origin der Installation. Um Personen von einer anderen Website zum Identitätsanbieter zu schicken, stattdessen auf `GET auth/oidc/start` verlinken; siehe [Anmeldung und Single Sign-on](../self-hosting/sign-in.md#links-die-direkt-zum-anbieter-führen).

## Fehler

```json
{
  "error": {
    "type": "permission_denied",
    "message": "You need the right `models.manage` in this project; ask one of its admins for it.",
    "right": "models.manage"
  }
}
```

| Status | `type` |
| --- | --- |
| 400 | `invalid_request` |
| 401 | `authentication` |
| 403 | `permission_denied` oder `recent_sign_in_required` |
| 404 | `not_found` |
| 405 | `invalid_request`, mit `Allow` |
| 409 | `conflict` oder `org_scheduled_for_deletion` für eine Änderung in einer Organisation, die auf ihre Löschung wartet |
| 412 | `conflict`, wenn `If-Match` eine ältere Revision nennt |
| 413 | `request_too_large`, für einen Body größer als `KAESO_BODY_LIMITS_PLATFORM_API`, standardmäßig 1 MiB |
| 429 | `rate_limited`, mit `Retry-After` |
| 500 | `internal`, mit der Anfrage-ID in der Meldung: „Something went wrong in the gateway; report request req_… to your admin.“ |
| 502, 503, 504, 429 | `provider_error`, nur für die Modellanfragen des Playgrounds |
| 503 | `unavailable`, etwa „Kaeso cannot reach its database right now; try again shortly.“ bei einem Ausfall der Datenbank |

`param` nennt das fehlerhafte Feld, den Query- oder Pfadparameter, und `right` das Recht, das dem Aufrufer fehlt, wenn ein einzelnes Recht die Anfrage erlauben würde; beide fehlen, wenn sie nicht zutreffen. Ein Parameter mit falschem Typ oder falscher Form oder ein Body, der kein JSON ist, ergibt `400 invalid_request`, ein unbekannter Pfad `404 not_found`. Die Meldung ist ein kurzer Satz für Menschen; die Konsole zeigt sie unverändert.

## Ein Durchgang mit curl

Diese Anfragen melden sich mit einem Passwort an und nutzen dann das Sitzungscookie. Dafür wird `jq` gebraucht.

```bash
KAESO=https://kaeso.example.com
JAR=$(mktemp)
```

Anmelden. Der erste Benutzer einer Installation registriert sich stattdessen über den Einrichtungslink.

```bash
curl -s -c "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"email": "owner@example.com", "password": "correct horse battery"}' \
  "$KAESO/platform/v1/auth/password"
```

Mit eingeschalteter Zwei-Faktor-Authentifizierung lautet die Antwort `202` mit einer `challenge`. Mit `POST /platform/v1/auth/totp` und `{"challenge": "…", "code": "123456"}` abschließen, oder mit einem `recovery_code` anstelle des Codes.

Die Organisation aus der Sitzung nehmen und ein Projekt erstellen:

```bash
ORG=$(curl -s -b "$JAR" "$KAESO/platform/v1/auth/session" | jq -r '.memberships[0].org_id')

PROJECT=$(curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"name": "Support bot"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects" | jq -r .id)
```

Den Anthropic-Schlüssel der Organisation hinzufügen. Das Secret wird nie wieder angezeigt; nur seine letzten vier Zeichen bleiben sichtbar. Der erste Schlüssel eines Anbieters ist sein Standard, den jedes Projekt ohne zugewiesenen Schlüssel nutzt.

```bash
curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"provider": "anthropic", "name": "Production", "secret": "sk-ant-…"}' \
  "$KAESO/platform/v1/orgs/$ORG/credentials"
```

Ein Modell zum Katalog des Projekts hinzufügen. Seine Werte kommen von models.dev.

```bash
curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"provider": "anthropic", "model_id": "claude-haiku-4-5"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/models"
```

Einen API-Schlüssel für die App erstellen. Das `secret` der Antwort ist der Schlüssel, einmal angezeigt.

```bash
KEY=$(curl -s -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"name": "support-bot", "expires_in_days": 90}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/keys" | jq -r .secret)
```

Die Modell-API mit einem offiziellen SDK aufrufen, oder mit curl:

```bash
curl -s "$KAESO/v1/messages" -H "x-api-key: $KEY" -H 'anthropic-version: 2023-06-01' \
  -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 100, "messages": [{"role": "user", "content": "Hello"}]}'
```

Nutzung und Kosten lesen, nach Tag (Standard), `week`, `month`, `model`, `key` oder `member`. `usage/span` liefert den ersten und letzten Tag mit Nutzung, `{"first_day": "2026-08-15", "last_day": "…"}`, um einen langen Zeitraum nach Woche oder Monat zu lesen. Nach Tag liegen `from` und `to` höchstens 400 Tage auseinander.

```bash
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage?group_by=model"
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage/span"
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage?from=2026-08-15&group_by=week"
```

Das Anfrageprotokoll lesen, neueste zuerst, und einen Eintrag mit seinem Inhalt. Der Inhalt erfordert das Recht `logs.content.view`.

```bash
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/logs?limit=10"
curl -s -b "$JAR" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/logs/req_…"
```

Die Aufrufe im Projekt funktionieren auch mit einem Schlüssel mit Plattformzugriff: Cookie und `Origin` durch `-H "Authorization: Bearer $KAESO_PLATFORM_KEY"` ersetzen. Jeder braucht sein Recht: `models.manage`, `keys.own.create`, `usage.view` und `logs.metadata.view`. Einen Anbieter-Schlüssel hinzuzufügen und ein Projekt zu erstellen sind Organisationsrechte, die ein Projektschlüssel nie hat; dafür einen Plattform-Schlüssel oder eine Sitzung verwenden.

## Bereiche

| Pfade | Was |
| --- | --- |
| `auth/*` | Einrichtung beim ersten Start, Anmeldemethoden, Passwort und Zwei-Faktor, Anmeldelinks, Zurücksetzen des Passworts, Single Sign-on, Sitzungen, Registrierung, wo die Instanz die Adresse zulässt, Einladungen |
| `me`, `me/*` | Das eigene Profil, Bild und Sprache (`locale`: `auto`, `en`, `de`, `zh-CN`), E-Mail-Adresse, Passwort, Zwei-Faktor und Wiederherstellungscodes, Sitzungen, Mitgliedschaften, die eigenen Schlüssel, Benachrichtigungen, eine Organisation erstellen, das Konto löschen |
| `orgs/{org}/*` | Die Organisation und ihr Bild, Mitglieder, Einladungen, Rollen und Rechte, Projekte und das Standardprojekt, Anbieter-Schlüssel (mehrere pro Anbieter, ein Standard, Zuweisungen zu Projekten), Sicherheit (Single Sign-on, verlangtes Single Sign-on und verlangte Zwei-Faktor-Authentifizierung, Schlüssellaufzeit), Plattform-Schlüssel, Protokolleinstellungen, Speicher und Löschungen, Nutzung, das Audit-Log, Übertragen der Inhaberschaft, Löschen und Wiederherstellen |
| `orgs/{org}/projects/{project}/*` | Das Projekt und sein Bild, Mitglieder und ihr Modellzugriff, Dienstkonten, Schlüssel mit ihren Modellen und Limits, Schlüsselrichtlinie, der Katalog (Suche, Vorschau, Stapel) und Aliasse, Limits, Inhaltsprotokollierung, Anfrageprotokolle und Exporte, Nutzung, das Audit-Log, der [Playground](#playground) |
| `instance/*` | Benutzer mit ihren Einladungen, Sitzungen und dem Flag als Instanz-Admin, Organisationen mit ihren Mitgliedern, Projekten, Nutzung und Speicher, Einstellungen mit einer Test-E-Mail, Status und Synchronisierung von models.dev, Systemzustand, Speicher und Protokollpuffer, das Audit-Log der Instanz; für Instanz-Admins |

Instanz-Admins sehen die Metadaten jeder Organisation, nie ihre Inhalte. Sie erstellen, benennen um, merken zur Löschung vor, stellen wieder her und löschen Organisationen unter `instance/orgs` und machen jeden aktiven Benutzer mit `POST instance/orgs/{org}/owner` zum Inhaber, sich selbst eingeschlossen; das ist ihr Weg, eine Organisation zu verwalten, und es wird in deren Audit-Log festgehalten. Nur sie setzen die Zwei-Faktor-Authentifizierung eines Benutzers zurück, mit `POST instance/users/{user}/reset-2fa`.

Es gibt keine Abrechnungsrouten. Die Rechte `billing.view` und `billing.manage` sind für Kaeso Cloud und tragen in `orgs/{org}/rights` das Flag `cloud_only`.

## Rollen und Rechte

Jedes Mitglied einer Organisation hat eine Organisationsrolle, und jedes Projektmitglied und Dienstkonto eine Projektrolle, jeweils mit einzeln hinzugefügten oder entzogenen Rechten. Rollen sind integriert oder eigene Rollen der Organisation (`role_…`). Eine Organisationsrolle enthält auch Rechte in jedem Projekt. [Rechte](../concepts/rights.md) listet jedes Recht und jede integrierte Rolle.

| Route | Recht | Was |
| --- | --- | --- |
| `GET orgs/{org}/rights` | `org.view` | Jedes Recht nach Kategorie, mit Name, Beschreibung, Flags (`sensitive`, `recent_sign_in`, `cloud_only`) und den Rechten, die es erfordert |
| `GET orgs/{org}/roles`, `GET …/roles/{role}` | `org.view` | Integrierte und eigene Rollen mit ihren Rechten, Rechten in jedem Projekt und der Zahl ihrer Inhaber; `?scope=org` oder `project` |
| `POST orgs/{org}/roles`, `PATCH …/roles/{role}`, `DELETE …/roles/{role}` | `roles.manage` | Eigene Rollen erstellen, ändern und löschen, höchstens 50; eine Änderung gilt sofort für alle, die die Rolle haben, und wird mit `400` abgelehnt, wenn danach jemand von ihnen ein Recht ohne ein dafür erforderliches Recht hätte, und eine Rolle, die jemand innehat, lässt sich nicht löschen |
| `GET orgs/{org}/members/{user}/rights` | `members.view` | Die wirksamen Organisationsrechte eines Mitglieds, seine Rechte in jedem Projekt und seine Rechte in jedem seiner Projekte, die der Aufrufer sehen kann, jeweils mit ihren Quellen |
| `GET orgs/{org}/projects/{project}/members/{user}/rights` | `project.members.view` | Die wirksamen Rechte eines Mitglieds in diesem Projekt, mit Quellen |
| `GET orgs/{org}/projects/{project}/my-rights` | `project.view` | Die eigenen Rechte des Aufrufers in diesem Projekt, mit Quellen |
| `GET orgs/{org}/rights/{right}/grantors`, `GET …/projects/{project}/rights/{right}/grantors` | `members.view`, `project.view` | Bis zu 10 aktive Mitglieder, die das Recht haben und Rechte ändern dürfen, Inhaber zuerst: wen man fragen kann |
| `GET orgs/{org}/projects/{project}/audit-log` | `project.audit_log.view` | Die Änderungen im Projekt |

Vergaben werden als `{"role": "developer", "rights_added": [], "rights_removed": []}` gesendet, wobei `role` der Name einer integrierten Rolle oder die ID einer eigenen Rolle ist; `preset` ist der veraltete Name dieses Felds. Sie kommen zurück mit `role` (`{id, kind, name}`), den wirksamen `rights` und, bei Organisationsvergaben, `project_rights`. Einladungen nehmen `role` und `project_roles: [{project_id, role}]`; die Einrichtung von Single Sign-on nimmt `default_role`.

Jede Änderung von Rechten folgt den Vergaberegeln:

- Sie gibt nur Rechte, die der Aufrufer hat: Organisationsrechte in der Organisation, Projektrechte in diesem Projekt, Rechte in jedem Projekt nur, wenn der Aufrufer sie in jedem Projekt hat.
- Sie ändert oder entfernt nur Mitglieder, deren Rechte der Aufrufer alle hat.
- Nur Inhaber machen, ändern, entfernen oder laden Inhaber ein.
- Jedes Recht, das sie gibt, kommt mit den Rechten, die es erfordert, sonst lautet die Antwort `400`, etwa „`logs.delete` needs `logs.storage.view`.“
- Sie entfernt kein Recht, das ein anderes Recht erfordert, das in der Vergabe oder Rolle bleibt, sonst lautet die Antwort `400`, etwa „`logs.delete` needs `logs.storage.view`; remove `logs.delete` too.“

Die Quellen eines wirksamen Rechts sind `owner`, `role` (die Organisationsrolle, für Organisationsrechte), `project_role`, `added`, `org_role` (die Rechte der Organisationsrolle in jedem Projekt), `org_right` (ein Organisationsrecht, das in Projekte hineinreicht, etwa `projects.view.all`) und `key` (ein API-Schlüssel, der im Projekt handelt).

## Playground

Der [Playground](../console/playground.md) der Konsole sendet Anfragen an die Modelle des Projekts als das angemeldete Mitglied. Beide Routen erfordern das Projektrecht `playground.use` und eine Sitzung; API-Schlüssel und Plattform-Schlüssel werden mit `403` abgelehnt, da Apps Modelle über `/v1` aufrufen.

| Route | Was |
| --- | --- |
| `POST orgs/{org}/projects/{project}/playground/responses` | Eine OpenAI-Responses-Anfrage mit `"stream": true` (`model`, `input` als String oder Input-Items, `instructions`, `temperature`, `top_p`, `max_output_tokens`, `reasoning.effort`, `reasoning.summary`); die Antwort ist der Responses-Event-Stream, den `/v1/responses` sendet, mit Keepalive-Kommentaren |
| `GET orgs/{org}/projects/{project}/playground/models` | Die Modelle, die das Mitglied dort aufrufen kann: der Katalog ohne Embedding-Modelle, eingeschränkt durch den Modellzugriff des Mitglieds, mit dem zu sendenden Namen, Aliassen, ob es Reasoning nutzt, seinen Aufwandsstufen, ob es `temperature` annimmt, Limits, Preisen und ob das Projekt einen Anbieter-Schlüssel dafür hat |

Eine Playground-Anfrage ist eine Modellanfrage wie jede andere, vom Mitglied ohne Schlüssel gestellt:

- **Derselbe Weg.** Das Modell wird im Katalog des Projekts aufgelöst, Aliasse und `provider/model` eingeschlossen, und gegen den Modellzugriff des Mitglieds geprüft. Der Anbieter-Schlüssel des Projekts wird verwendet, und das Schließen des Streams bricht die Anfrage beim Anbieter ab.
- **Limits.** Raten und Budgets des Mitglieds und des Projekts gelten; Schlüssel-Limits nicht, da es keinen Schlüssel gibt.
- **Nutzung und Protokolle.** Die Anfrage wird dem Projekt berechnet, dem Mitglied zugerechnet (Nutzung nach `key` zeigt sie als `playground`) und mit der Quelle `playground` protokolliert, mit Inhalt, wenn das Projekt Inhalte protokolliert. `logs` und `logs/export` filtern nach `source=api` oder `source=playground`.
- **Fehler.** Vor dem Start des Streams sind Fehler Platform-API-Fehler: `400 invalid_request` (`param` ist `model` für ein Modell außerhalb des Katalogs, `stream` für `"stream": false`), `403 permission_denied` für ein Modell außerhalb des Modellzugriffs des Mitglieds, `409 conflict` für ein archiviertes Projekt oder einen Anbieter ohne Schlüssel, `409 org_scheduled_for_deletion`, `429 rate_limited` mit dem Namen des Limits, `503 unavailable` und `provider_error`, wenn der Anbieter fehlschlägt oder den Anbieter-Schlüssel ablehnt (`502`), ein Ratenlimit setzt (`429`), überlastet ist (`503`) oder nicht rechtzeitig antwortet (`504`). Nach dem Start des Streams sind Fehler `error`-Events von Responses, wie auf `/v1`.

```bash
curl -N -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "input": "Hello", "stream": true, "reasoning": {"summary": "auto"}}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/playground/responses"
```

## Wie die Konsole sie nutzt

Die Konsole läuft auf demselben Origin wie das Gateway, das Sitzungscookie ist also First-Party, und es gibt kein CORS. Sie:

- meldet Personen über `auth/*` an und behält nur das HttpOnly-Cookie;
- sendet `Origin` mit jeder Änderung, wie Browser es von selbst tun;
- liest `auth/session` für den angemeldeten Benutzer, seine Mitgliedschaften mit Organisationsrolle, Rechten und Rechten in jedem Projekt, ob er Instanz-Admin ist und ob er Organisationen erstellen darf (`can_create_orgs`);
- liest die `my-rights` eines Projekts, um zu entscheiden, was dort angezeigt wird, und nimmt Namen, Gruppen und Abhängigkeiten der Rechte aus `orgs/{org}/rights`;
- blättert Listen mit den oben beschriebenen Cursorn und füllt die Modellformulare aus `models/search` und `models/preview`;
- zeigt `recent_sign_in_required` als Aufforderung zur erneuten Anmeldung und wiederholt dann die Aktion;
- lädt Exporte (`logs/export`, `audit-log/export`) als gestreamte Dateien herunter;
- streamt Antworten im Playground mit `fetch`; **Stoppen** bricht den Fetch ab, was die Anfrage beim Anbieter abbricht;
- zeigt Nutzung nach Tag bis etwa drei Monate, nach Woche bis etwa ein Jahr und darüber nach Monat, und beginnt **Seit Beginn** am `first_day` von `usage/span`.
