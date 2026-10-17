---
title: Platform API
description: Eine Installation über HTTP verwalten, so wie die Konsole es tut.
order: 3
---

Die Platform API verwaltet Organisationen, Projekte, Mitglieder, Schlüssel, Anbieter-Schlüssel, Modelle, Limits, Nutzung und Protokolle. Sie liegt unter `/platform/v1` auf derselben Domain wie die Modell-API. Die Konsole nutzt nichts anderes, also kann ein Skript alles, was die Konsole kann.

Die vollständige Referenz ist das OpenAPI-Dokument [`openapi.json`](https://github.com/logicplanes/kaeso/blob/dev/openapi.json), erzeugt aus dem Code des Gateways. Jede Operation darin nennt das eine Recht, das sie prüft. Das Dokument der laufenden Version ausgeben:

```bash
docker compose exec gateway kaeso openapi > openapi.json
```

## Authentifizierung

| Methode | Senden | Handelt mit |
| --- | --- | --- |
| Sitzung | Das Sitzungs-Cookie, das die Anmelde-Endpunkte unter `auth/*` setzen | Den Rechten des angemeldeten Benutzers |
| API-Schlüssel mit Plattformzugriff | `Authorization: Bearer sk-kso-…` | Den Rechten seines Mitglieds oder Dienstkontos, nur im eigenen Projekt |
| Plattform-Schlüssel | `Authorization: Bearer sk-kso-org-…` | Den Rechten der Rolle Admin, in seiner Organisation und jedem Projekt darin |

Sitzungen dienen der Konsole und Personen mit curl. Das Cookie ist das HttpOnly-Cookie `__Host-kaeso_session`, bei einer `http://`-Installation unter einer Loopback-Adresse wie localhost `kaeso_session`. Projektschlüssel passen zu Skripten und CI für ein Projekt, Plattform-Schlüssel zu Automatisierung für eine ganze Organisation.

- **Schlüssel erreichen nur `orgs/{org}/*`.** `me/*`, `instance/*` und die angemeldeten Routen von `auth/*` lehnen sie mit 403 ab, ebenso jede Route auf Organisationsebene einen Projektschlüssel. Die Platform API liest Schlüssel nur aus `Authorization`, nie aus `x-api-key`.
- **Plattformzugriff wird beim Erstellen eines Schlüssels gewählt**, mit `"access": "platform"` oder `"both"`; Standard ist `inference`. Er erfordert das Recht `keys.platform_access.grant`. Siehe [API-Schlüssel](../console/api-keys.md).
- **Plattform-Schlüssel entstehen nur in einer Sitzung**, durch jemanden, der jedes Recht der Rolle Admin hat. Sie rufen nie Modelle auf.
- **Richtlinien der Organisation gelten für Sitzungen.** Eine Organisation, die Single Sign-on verlangt, lehnt andere Sitzungen mit 403 ab; eine, die Zwei-Faktor-Authentifizierung verlangt, lehnt Passwortsitzungen ohne zweiten Faktor ab.
- **Unbekannte Schlüssel werden gebremst.** Zu viele Anfragen mit unbekannten API-Schlüsseln von einer Adresse, auf Modell-API und Platform API zusammen, erhalten `429 rate_limited` mit `Retry-After`, bis das Zeitfenster endet; [Fehlgeschlagene API-Schlüssel](../self-hosting/configuration.md#fehlgeschlagene-api-schlüssel) nennt die Einstellungen.

### Websiteübergreifende Anfragen

Jede Anfrage außer `GET`, `HEAD` und `OPTIONS`, die das Sitzungs-Cookie trägt, und jede solche Anfrage an einen öffentlichen `auth/*`-Endpunkt muss `Origin` mit dem Origin der öffentlichen URL der Installation senden, etwa `Origin: https://kaeso.example.com`. Browser senden ihn selbst; `Referer` zählt nicht an seiner Stelle. Ohne ihn oder mit einem anderen Origin lautet die Antwort `403 permission_denied`, und die Meldung nennt den zu sendenden Header. So können andere Websites weder mit der Sitzung einer Person handeln noch einen Browser in ein eigenes Konto anmelden.

- Anfragen mit einem API-Schlüssel oder Plattform-Schlüssel brauchen kein `Origin`.
- Lesende Anfragen brauchen es nie, auch nicht `GET auth/methods` und `GET auth/oidc/start`.
- Ein Server, der `POST auth/methods`, `POST auth/password` oder `POST auth/oidc/start` aufruft, sendet den Origin der Installation. Um Personen von einer anderen Website zum Identitätsanbieter zu schicken, stattdessen auf `GET auth/oidc/start` verlinken; siehe [Anmeldung und Single Sign-on](../self-hosting/sign-in.md#links-die-direkt-zum-anbieter-führen).

### Kürzliche Anmeldung

In einer Sitzung verlangen einige Aktionen eine Anmeldung innerhalb der letzten 15 Minuten; sonst lautet die Antwort `403 recent_sign_in_required`, und die Konsole bittet um eine erneute Anmeldung. Schlüssel werden nie gefragt.

- Operationen, deren Recht in `orgs/{org}/rights` das Kennzeichen `recent_sign_in` trägt: `org.delete`, `org.ownership.transfer`, `provider_credentials.create`, `provider_credentials.replace`, `security.sso.manage`, `security.sso.require`, `platform_keys.create`, `logs.delete` und `keys.platform_access.grant`.
- Im eigenen Konto: E-Mail-Adresse oder Benachrichtigungen ändern, Zwei-Faktor-Authentifizierung abschalten, neue Wiederherstellungscodes, das Konto löschen.
- Für Instanz-Admins: Instanzeinstellungen ändern und eine Organisation zur Löschung vormerken, wiederherstellen oder löschen.

## Konventionen

| Thema | Regel |
| --- | --- |
| Format | REST mit JSON. Pfade im Plural in Kebab-Case (`service-accounts`), Felder in `snake_case`. Bodys werden mit `content-type: application/json` gesendet; ein Feld, das der Endpunkt nicht kennt, wird mit 400 abgelehnt |
| IDs | Ein Präfix und ein zeitlich geordneter Teil: `org_…`, `proj_…`, `usr_…`, `key_…`, `opk_…`, `svc_…`, `cred_…`, `model_…`, `alias_…`, `lim_…`, `role_…`, `inv_…`, `ses_…`, `req_…`, `evt_…` |
| Zeiten | RFC 3339, in UTC |
| Geld | US-Dollar als Dezimal-Strings, etwa `"0.0001"`. Preise und Beträge werden als Dezimal-Strings oder ganze Zahlen gesendet; ein JSON-Bruch wie `0.5` wird mit 400 abgelehnt |
| Beträge von Limits | Höchstens 10^12 Anfragen oder Token, bei einem Ausgabenbudget 1.000.000.000 USD |
| Erstellen | `POST` auf die Sammlung, beantwortet mit 201 und der Ressource |
| Ändern | `PATCH` nur mit den zu ändernden Feldern |
| Aktionen | `POST …/archive`, `…/disable`, `…/enable`, `…/replace`, `…/restore`; Archivieren behält den Datensatz |
| Löschen | `DELETE`, nur wo wirklich etwas gelöscht wird: Anbieter-Schlüssel, Aliasse, Katalogmodelle, Limits, eigene Rollen, Sitzungen, Einladungen, das Konto |
| Audit | Jede Änderung wird in derselben Transaktion in das Audit-Log ihrer Ebene geschrieben |
| Anfrage-IDs | Jede Antwort trägt `kaeso-request-id` |

### Listen

Eine Liste nimmt `limit` (1 bis 100, Standard 20) und `after`, den `next_cursor` der vorigen Seite; Filter sind weitere Query-Parameter. Listen sind nach ID sortiert, also in Erstellungsreihenfolge; Anfrageprotokolle und Audit-Logs kommen neueste zuerst.

```json
{ "data": [], "has_more": true, "next_cursor": "…" }
```

Auf der letzten Seite ist `next_cursor` `null`.

### Revisionen

Katalogmodelle und Limits tragen eine `revision`, die mit jeder Änderung steigt, und Lesen, Erstellen oder Ändern antwortet mit `ETag: "<revision>"`. Wer dieses Tag mit `PATCH` oder `DELETE` in `If-Match` zurückschickt, ändert nur die gelesene Revision: Hat sich der Eintrag inzwischen geändert, lautet die Antwort `412 conflict`. Ohne `If-Match` gilt die Änderung für die aktuelle Revision.

### Zur Löschung vorgemerkte Organisationen

Eine zur Löschung vorgemerkte Organisation ist schreibgeschützt, bis sie wiederhergestellt oder gelöscht ist. Lesen funktioniert, soweit die Rechte des Aufrufers reichen. Jede Änderung darin, auch sie zu verlassen und eine Einladung in sie anzunehmen, ergibt `409 org_scheduled_for_deletion`, außer `POST orgs/{org}/restore` und `instance/orgs/{org}/*` der Instanz-Admins. Ihre Schlüssel authentifizieren gar nicht.

`auth/session` zeigt eine solche Organisation in jeder Mitgliedschaft mit `org_state`, `org_deletion_due_at` und Rechten, reduziert auf Sehen, Exportieren und `org.delete`. Es enthält auch die Richtlinie `org_deletion` der Instanz (`allowed`, `grace_days`).

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

Die Meldung ist ein kurzer Satz für Menschen; die Konsole zeigt sie unverändert. `param` nennt das fehlerhafte Body-Feld oder den Query- oder Pfad-Parameter, `right` das fehlende Recht, wenn genau dieses Recht die Anfrage erlauben würde; beide fehlen, wo sie nicht zutreffen.

| Status | `type` | Wann |
| --- | --- | --- |
| 400 | `invalid_request` | Ein Body, der kein JSON ist, ein unbekanntes Feld, ein Parameter mit falschem Typ oder falscher Form, ein Wert außerhalb des Bereichs |
| 401 | `authentication` | Keine Sitzung und kein Schlüssel, oder eine bzw. einer, die unbekannt, abgelaufen oder beendet ist |
| 403 | `permission_denied` | Ein fehlendes Recht, ein Schlüssel auf einer Route, die er nicht nutzen kann, ein fehlender `Origin`, eine Richtlinie der Organisation |
| 403 | `recent_sign_in_required` | Siehe [Kürzliche Anmeldung](#kürzliche-anmeldung) |
| 404 | `not_found` | Ein unbekannter Pfad, oder etwas, das es nicht gibt oder das der Aufrufer nicht sehen kann |
| 405 | `invalid_request` | Eine Methode, die der Pfad nicht annimmt; `Allow` nennt die möglichen |
| 409 | `conflict` | Die Änderung widerspricht dem aktuellen Zustand, etwa ein schon vergebener Name |
| 409 | `org_scheduled_for_deletion` | Eine Änderung in einer Organisation, die auf ihre Löschung wartet |
| 412 | `conflict` | `If-Match` nennt eine ältere Revision |
| 413 | `request_too_large` | Der Body ist größer als `KAESO_BODY_LIMITS_PLATFORM_API`, standardmäßig 1 MiB |
| 429 | `rate_limited` | Zu viele unbekannte Schlüssel, oder ein Limit bei einer Playground-Anfrage; `Retry-After` sagt, wann es wieder geht |
| 500 | `internal` | Ein Fehler im Gateway; die Meldung nennt die Anfrage-ID, die zu melden ist |
| 502, 503, 504, 429 | `provider_error` | Nur bei den Modellanfragen des Playgrounds |
| 503 | `unavailable` | Eine Datenbank oder ein anderer Hintergrunddienst ist nicht erreichbar, etwa „Kaeso cannot reach its database right now; try again shortly.“ |

## Ein Durchgang mit curl

Diese Anfragen richten ein Projekt für eine App ein und rufen darüber ein Modell auf, mit einem Plattform-Schlüssel. Den Schlüssel in der Konsole in den Einstellungen der Organisation unter **Plattform-Schlüssel** erstellen; die ID der Organisation steht in der Adresse der Konsole, `/orgs/org_…`. Die Befehle brauchen `jq`.

```bash
KAESO=https://kaeso.example.com
ORG=org_…
AUTH="Authorization: Bearer sk-kso-org-…"
```

Ein Projekt erstellen:

```bash
PROJECT=$(curl -s -H "$AUTH" -H 'content-type: application/json' \
  -d '{"name": "Support bot"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects" | jq -r .id)
```

Den Anthropic-Schlüssel der Organisation hinzufügen. Der erste Schlüssel eines Anbieters wird dessen Standard, den jedes Projekt ohne zugewiesenen Schlüssel nutzt. Das Geheimnis wird nie wieder angezeigt; sichtbar bleiben nur die letzten vier Zeichen.

```bash
curl -s -H "$AUTH" -H 'content-type: application/json' \
  -d '{"provider": "anthropic", "name": "Production", "secret": "sk-ant-…"}' \
  "$KAESO/platform/v1/orgs/$ORG/credentials"
```

Ein Modell in den Katalog des Projekts aufnehmen. Preise, Limits und Fähigkeiten kommen von models.dev.

```bash
curl -s -H "$AUTH" -H 'content-type: application/json' \
  -d '{"provider": "anthropic", "model_id": "claude-haiku-4-5"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/models"
```

Ein Dienstkonto für die App mit der Rolle Entwickler anlegen und einen Schlüssel dafür, der nach 90 Tagen abläuft. Das `secret` der Antwort ist der API-Schlüssel, einmal angezeigt.

```bash
ACCOUNT=$(curl -s -H "$AUTH" -H 'content-type: application/json' \
  -d '{"name": "support-bot", "role": "developer"}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/service-accounts" | jq -r .id)

KEY=$(curl -s -H "$AUTH" -H 'content-type: application/json' \
  -d '{"name": "support-bot", "expires_in_days": 90}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/service-accounts/$ACCOUNT/keys" | jq -r .secret)
```

Die Modell-API mit dem neuen Schlüssel aufrufen, wie es die App tun wird:

```bash
curl -s "$KAESO/v1/messages" -H "x-api-key: $KEY" -H 'anthropic-version: 2023-06-01' \
  -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "max_tokens": 100, "messages": [{"role": "user", "content": "Hello"}]}'
```

Die Nutzung lesen, gruppiert nach `day` (Standard), `week`, `month`, `model`, `key` oder `member`; Kosten sind mit dem Recht `costs.view` enthalten. `from` und `to` sind Tage in UTC; nach Tagen liegen sie höchstens 400 Tage auseinander. `usage/span` nennt den ersten und letzten Tag mit Nutzung, um einen langen Zeitraum nach Woche oder Monat zu lesen.

```bash
curl -s -H "$AUTH" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage?group_by=model"
curl -s -H "$AUTH" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage/span"
curl -s -H "$AUTH" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/usage?from=2026-08-15&group_by=week"
```

Das Anfrageprotokoll lesen, neueste zuerst, dann einen Eintrag. Der Eintrag enthält den Inhalt der Anfrage, wenn das Projekt Inhalte protokolliert und der Aufrufer `logs.content.view` hat.

```bash
curl -s -H "$AUTH" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/logs?limit=10"
curl -s -H "$AUTH" "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/logs/req_…"
```

### Mit einem Projektschlüssel

Ein API-Schlüssel mit Plattformzugriff funktioniert ebenso in seinem eigenen Projekt, mit den Rechten seines Mitglieds oder Dienstkontos: `models.manage`, um Modelle hinzuzufügen, `usage.view` und `logs.metadata.view` zum Lesen. Projekte erstellen und Anbieter-Schlüssel hinzufügen sind Rechte der Organisation, die ein Projektschlüssel nie hat. Ein Schlüssel, der einem Mitglied gehört, erstellt mit `POST …/projects/{project}/keys` weitere Schlüssel für dieses Mitglied.

### Mit einer Sitzung

Die Anmeldung mit Passwort funktioniert, solange sie eingeschaltet ist, wie standardmäßig. Das Cookie in einer Datei halten und bei jeder Änderung `Origin` senden:

```bash
KAESO=https://kaeso.example.com
JAR=$(mktemp)

curl -s -c "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"email": "owner@example.com", "password": "correct horse battery staple"}' \
  "$KAESO/platform/v1/auth/password"

ORG=$(curl -s -b "$JAR" "$KAESO/platform/v1/auth/session" | jq -r '.memberships[0].org_id')
```

Mit Zwei-Faktor-Authentifizierung antwortet `auth/password` mit 202 und einer `challenge`; abschließen mit `POST /platform/v1/auth/totp` und `{"challenge": "…", "code": "123456"}`, oder mit `recovery_code` anstelle von `code`. Der erste Benutzer einer neuen Installation registriert sich über den Einrichtungslink, den `kaeso setup-link` ausgibt; siehe [Installation](../self-hosting/installation.md).

Danach funktionieren die Anfragen oben mit `-b "$JAR" -H "Origin: $KAESO"` statt `-H "$AUTH"`. Eine Sitzung erstellt mit `POST …/projects/{project}/keys` auch Schlüssel für den eigenen Benutzer.

## Bereiche

| Pfade | Inhalt |
| --- | --- |
| `auth/*` | Ersteinrichtung, Anmeldemethoden, Passwort und Zwei-Faktor, Anmeldelinks, Passwort zurücksetzen, Single Sign-on, Sitzungen, Registrierung, wo die Instanz die Adresse zulässt, Einladungen |
| `me`, `me/*` | Das eigene Profil, Bild und Sprache (`locale`: `auto`, `en`, `de`, `zh-CN`), E-Mail-Adresse, Passwort, Zwei-Faktor und Wiederherstellungscodes, Sitzungen, Mitgliedschaften, eigene Schlüssel, Benachrichtigungen, eine Organisation erstellen, das Konto löschen |
| `orgs/{org}/*` | Die Organisation und ihr Bild, Mitglieder, Einladungen, Rollen und Rechte, Projekte und das Standardprojekt, Anbieter-Schlüssel (mehrere je Anbieter, ein Standard, Zuweisungen an Projekte), Sicherheit (Single Sign-on, verlangtes Single Sign-on und Zwei-Faktor, Laufzeit von Schlüsseln), Plattform-Schlüssel, Protokolleinstellungen, Speicher und Löschungen, Nutzung, das Audit-Log, Übertragung der Inhaberschaft, Löschen und Wiederherstellen |
| `orgs/{org}/projects/{project}/*` | Das Projekt und sein Bild, Mitglieder und ihr Modellzugriff, Dienstkonten, Schlüssel mit ihren Modellen und Limits, Schlüsselrichtlinie, der Katalog (Suche, Vorschau, Stapel) und Aliasse, Limits, Inhaltsprotokollierung, Anfrageprotokolle und Exporte, Nutzung, das Audit-Log, der [Playground](#playground) |
| `instance/*` | Für Instanz-Admins: Benutzer, ihre Einladungen, Sitzungen und Admin-Kennzeichen; Organisationen mit Mitgliedern, Projekten, Nutzung und Speicher; Einstellungen mit Test-E-Mail; Status und Abgleich von models.dev; Systemzustand, Speicher und Protokollpuffer; Updates: die neuesten Releases, ihre Einstellungen, das Einspielen über den Updater und sein Fortschritt; das Audit-Log der Instanz |

Instanz-Admins sehen die Metadaten jeder Organisation, nie ihre Inhalte. Sie erstellen, benennen um, merken zur Löschung vor, stellen wieder her und löschen Organisationen unter `instance/orgs`. `POST instance/orgs/{org}/owner` macht jeden aktiven Benutzer zum Inhaber, den Admin selbst eingeschlossen; so verschafft sich ein Instanz-Admin Zugang, um eine Organisation zu verwalten, und das Audit-Log der Organisation hält es fest. Nur Instanz-Admins setzen die Zwei-Faktor-Authentifizierung eines Benutzers zurück, mit `POST instance/users/{user}/reset-2fa`.

Abrechnungsrouten gibt es nicht. Die Rechte `billing.view` und `billing.manage` sind für Kaeso Cloud bestimmt und tragen in `orgs/{org}/rights` das Kennzeichen `cloud_only`.

## Rollen und Rechte

Jedes Mitglied einer Organisation hat eine Organisationsrolle, jedes Projektmitglied und Dienstkonto eine Projektrolle, jeweils mit einzeln hinzugefügten oder entfernten Rechten. Eine Rolle ist eingebaut oder eine eigene Rolle der Organisation (`role_…`). Eine Organisationsrolle hält auch Rechte in jedem Projekt. [Rechte](../concepts/rights.md) listet jedes Recht und jede eingebaute Rolle.

| Route | Recht | Inhalt |
| --- | --- | --- |
| `GET orgs/{org}/rights` | `org.view` | Jedes Recht nach Kategorie, mit Name, Beschreibung, Kennzeichen (`sensitive`, `recent_sign_in`, `cloud_only`) und den Rechten, die es braucht |
| `GET orgs/{org}/roles`, `GET …/roles/{role}` | `org.view` | Eingebaute und eigene Rollen mit ihren Rechten, Rechten in jedem Projekt und Zahl der Inhaber; `?scope=org` oder `project` |
| `POST orgs/{org}/roles`, `PATCH …/roles/{role}`, `DELETE …/roles/{role}` | `roles.manage` | Eigene Rollen erstellen, ändern und löschen, höchstens 50 je Organisation |
| `GET orgs/{org}/members/{user}/rights` | `members.view` | Die wirksamen Organisationsrechte eines Mitglieds, Rechte in jedem Projekt und Rechte in jedem seiner Projekte, die der Aufrufer sehen kann, jeweils mit Quellen |
| `GET orgs/{org}/projects/{project}/members/{user}/rights` | `project.members.view` | Die wirksamen Rechte eines Mitglieds in diesem Projekt, mit Quellen |
| `GET orgs/{org}/projects/{project}/my-rights` | `project.view` | Die eigenen Rechte des Aufrufers in diesem Projekt, mit Quellen |
| `GET orgs/{org}/rights/{right}/grantors`, `GET …/projects/{project}/rights/{right}/grantors` | `members.view`, `project.view` | Wen man nach einem Recht fragen kann: bis zu 10 aktive Mitglieder, die es haben und Rechte ändern dürfen, Inhaber zuerst |
| `GET orgs/{org}/projects/{project}/audit-log` | `project.audit_log.view` | Die Änderungen im Projekt |

Eine Vergabe wird als `{"role": "developer", "rights_added": [], "rights_removed": []}` gesendet, wobei `role` der Name einer eingebauten Rolle oder die ID einer eigenen Rolle ist; `preset` ist ihr veralteter Name. Zurück kommt sie mit `role` (`{id, kind, name}`), den wirksamen `rights` und, bei Organisationsvergaben, `project_rights`. Einladungen nehmen `role` und `project_roles: [{project_id, role}]`; die Einrichtung von Single Sign-on nimmt `default_role`.

Jede Änderung von Rechten, an einer Vergabe oder einer eigenen Rolle, folgt denselben Regeln:

- Sie vergibt nur Rechte, die der Aufrufer hat: Organisationsrechte in der Organisation, Projektrechte in diesem Projekt, Rechte in jedem Projekt nur, wenn der Aufrufer sie in jedem Projekt hat.
- Sie ändert oder entfernt nur Mitglieder, deren Rechte der Aufrufer alle hat. Nur Inhaber ernennen, ändern, entfernen oder laden Inhaber ein.
- Jedes vergebene Recht kommt mit den Rechten, die es braucht, sonst lautet die Antwort 400, etwa „`logs.delete` needs `logs.storage.view`.“
- Sie entfernt kein Recht, das ein verbleibendes Recht braucht, sonst lautet die Antwort 400, etwa „`logs.delete` needs `logs.storage.view`; remove `logs.delete` too.“ Eine Änderung an einer eigenen Rolle wird auch abgelehnt, wenn sie einen Inhaber in diesem Zustand zurückließe.
- Eine Änderung an einer eigenen Rolle gilt sofort für alle Inhaber, und eine Rolle, die jemand innehat, lässt sich nicht löschen.

Die Quellen eines wirksamen Rechts sind `owner`, `role` (die Organisationsrolle, bei Organisationsrechten), `project_role`, `added`, `org_role` (die Rechte der Organisationsrolle in jedem Projekt), `org_right` (ein Organisationsrecht, das in Projekte hineinreicht, etwa `projects.view.all`) und `key` (ein API-Schlüssel, der im Projekt handelt).

## Playground

Der [Playground](../console/playground.md) der Konsole sendet Anfragen an die Modelle des Projekts als das angemeldete Mitglied. Beide Routen brauchen eine Sitzung und das Projektrecht `playground.use`; Schlüssel werden mit 403 abgelehnt, da Apps Modelle über `/v1` aufrufen.

`GET orgs/{org}/projects/{project}/playground/models` listet die Modelle, die das Mitglied dort aufrufen kann: den Katalog ohne Embedding-Modelle, eingeschränkt durch den Modellzugriff des Mitglieds. Zu jedem gehören der zu sendende Name, seine Aliasse, ob es Reasoning beherrscht und mit welchen Stufen, ob es `temperature` annimmt, seine Limits und Preise und ob das Projekt einen Anbieter-Schlüssel dafür hat.

`POST orgs/{org}/projects/{project}/playground/responses` nimmt eine OpenAI-Responses-Anfrage mit `model`, `input` (ein String oder Input-Items), `instructions`, `temperature`, `top_p`, `max_output_tokens`, `reasoning.effort`, `reasoning.summary` und dem verpflichtenden `"stream": true`. Die Antwort ist der Responses-Event-Stream, den `/v1/responses` sendet, mit Keepalive-Kommentaren. Mit der Sitzung [von oben](#mit-einer-sitzung):

```bash
curl -N -b "$JAR" -H "Origin: $KAESO" -H 'content-type: application/json' \
  -d '{"model": "claude-haiku-4-5", "input": "Hello", "stream": true, "reasoning": {"summary": "auto"}}' \
  "$KAESO/platform/v1/orgs/$ORG/projects/$PROJECT/playground/responses"
```

Eine Playground-Anfrage ist eine Modellanfrage wie jede andere, gestellt vom Mitglied ohne Schlüssel:

- **Derselbe Weg.** Das Modell wird im Katalog des Projekts aufgelöst, Aliasse und `provider/model` eingeschlossen, und gegen den Modellzugriff des Mitglieds geprüft. Der Anbieter-Schlüssel des Projekts wird verwendet, und das Schließen des Streams bricht die Anfrage beim Anbieter ab.
- **Limits.** Die Raten und Budgets des Mitglieds und des Projekts gelten; Schlüssel-Limits nicht, da es keinen Schlüssel gibt.
- **Nutzung und Protokolle.** Die Anfrage wird dem Projekt berechnet und dem Mitglied zugerechnet; die Nutzung nach `key` zeigt sie als `playground`. Sie wird mit der Quelle `playground` protokolliert, mit Inhalt, wenn das Projekt Inhalte protokolliert; `logs` und `logs/export` filtern mit `source=api` oder `source=playground`.

Fehler vor Beginn des Streams sind Fehler der Platform API:

| Status | `type` | Ursache |
| --- | --- | --- |
| 400 | `invalid_request` | `param` ist `model` bei einem Modell außerhalb des Katalogs oder einem, das der Anbieter dem Anbieter-Schlüssel des Projekts nicht bereitstellt, und `stream` bei `"stream": false` |
| 403 | `permission_denied` | Das Modell liegt außerhalb des Modellzugriffs des Mitglieds |
| 409 | `conflict` | Das Projekt ist archiviert, oder für den Anbieter des Modells ist kein Anbieter-Schlüssel eingerichtet |
| 409 | `org_scheduled_for_deletion` | Die Organisation wartet auf ihre Löschung |
| 429 | `rate_limited` | Eine Rate oder ein Budget, in der Meldung genannt |
| 503 | `unavailable` | Katalog, Anbieter-Schlüssel oder Limits sind nicht lesbar, oder das Gateway startet neu |
| 502, 429, 503, 504 | `provider_error` | Der Anbieter ist gescheitert oder hat den Anbieter-Schlüssel abgelehnt, hat die Anfrage gebremst, ist überlastet oder hat nicht rechtzeitig geantwortet |

Nach Beginn des Streams kommen Fehler als Responses-`error`-Events, wie auf `/v1`.

## Wie die Konsole sie nutzt

Die Konsole wird vom Origin des Gateways selbst ausgeliefert, also ist ihr Sitzungs-Cookie First-Party und sie braucht kein CORS. Sie hält nur das HttpOnly-Cookie und sendet bei jeder Änderung `Origin`, wie es Browser tun. Darüber hinaus:

- liest sie `auth/session` für den Benutzer, seine Mitgliedschaften mit Organisationsrolle, Rechten und Rechten in jedem Projekt, ob er Instanz-Admin ist und ob er Organisationen erstellen darf (`can_create_orgs`);
- liest sie `my-rights` eines Projekts, um zu entscheiden, was sie dort zeigt, und nimmt Namen, Gruppen und Abhängigkeiten der Rechte aus `orgs/{org}/rights`;
- füllt sie die Modellformulare aus `models/search` und `models/preview`;
- zeigt sie `recent_sign_in_required` als Aufforderung zur erneuten Anmeldung und wiederholt danach die Aktion;
- lädt sie `logs/export` und `audit-log/export` als gestreamte Dateien herunter;
- streamt sie Playground-Antworten mit `fetch`, wobei **Stoppen** den Abruf abbricht und damit die Anfrage beim Anbieter;
- zeichnet sie die Nutzung bis etwa drei Monate nach Tagen, bis etwa ein Jahr nach Wochen und darüber nach Monaten und lässt **Seit Beginn** beim `first_day` von `usage/span` anfangen.
