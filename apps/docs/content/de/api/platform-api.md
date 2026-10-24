---
title: Platform API
description: Authentifizieren, Listen durchblättern, Fehler behandeln und die Routen finden, um eine Installation über HTTP zu verwalten.
order: 3
---

Die Platform API verwaltet alles, was die Konsole zeigt: Organisationen, Projekte, Mitglieder, Schlüssel, Anbieter-Schlüssel, Modelle, Limits, Nutzung und Protokolle. Sie liegt unter `/platform/v1` auf der Domain Ihrer Installation, und die Konsole nutzt nichts anderes.

Listen Sie die Projekte einer Organisation mit einem Plattform-Schlüssel in `KAESO_PLATFORM_KEY` auf, mit der ID Ihrer Organisation anstelle von `org_…`:

```bash tab="curl"
curl https://kaeso.example.com/platform/v1/orgs/org_…/projects \
  -H "Authorization: Bearer $KAESO_PLATFORM_KEY"
```

```python tab="Python"
import json
import os
import urllib.request

request = urllib.request.Request(
    "https://kaeso.example.com/platform/v1/orgs/org_…/projects",
    headers={"Authorization": f"Bearer {os.environ['KAESO_PLATFORM_KEY']}"},
)
with urllib.request.urlopen(request) as response:
    print(json.load(response))
```

```ts tab="TypeScript"
const url = "https://kaeso.example.com/platform/v1/orgs/org_…/projects";
const response = await fetch(url, {
  headers: { Authorization: `Bearer ${process.env.KAESO_PLATFORM_KEY}` },
});
console.log(await response.json());
```

Die Antwort ist eine Seite mit Projekten:

```json
{
  "data": [
    {
      "id": "proj_…",
      "org_id": "org_…",
      "name": "Support bot",
      "is_default": false,
      "content_logging": true,
      "created_at": "2026-10-01T09:30:00Z"
    }
  ],
  "has_more": false,
  "next_cursor": null
}
```

[Mit der Platform API automatisieren](automation.md) richtet Schritt für Schritt ein Projekt, ein Modell und den Schlüssel einer App ein.

## OpenAPI-Dokument

[`openapi.json`](https://github.com/logicplanes/kaeso/blob/dev/openapi.json) beschreibt jede Operation: ihren Pfad, ihre Felder, ihre Antworten und das eine Recht, das sie prüft. Das Gateway erzeugt das Dokument aus seinem eigenen Code. So erhalten Sie das Dokument der Version, die Sie betreiben:

```bash
docker compose exec gateway kaeso openapi > openapi.json
```

## Authentifizierung

| Methode | Senden | Handelt mit |
| --- | --- | --- |
| Plattform-Schlüssel | `Authorization: Bearer sk-kso-org-…` | Den Rechten der Rolle Admin, in seiner Organisation und allen ihren Projekten |
| API-Schlüssel mit Plattformzugriff | `Authorization: Bearer sk-kso-…` | Den Rechten seines Mitglieds oder Dienstkontos, nur im eigenen Projekt |
| Sitzung | Das Sitzungs-Cookie, das die Anmelde-Endpunkte unter `auth/*` setzen | Ihren eigenen Rechten |

Verwenden Sie einen Plattform-Schlüssel, um eine ganze Organisation zu automatisieren, und einen Projektschlüssel für Skripte und CI in einem Projekt. Sitzungen dienen der Konsole und Personen mit curl. Das Sitzungs-Cookie heißt `__Host-kaeso_session`, bei einer `http://`-Installation unter einer Loopback-Adresse wie localhost `kaeso_session`.

- **Schlüssel erreichen nur `orgs/{org}/*`.** `me/*`, `instance/*` und die Routen von `auth/*`, die eine Anmeldung voraussetzen, beantworten Schlüssel mit 403. Routen auf Organisationsebene beantworten Projektschlüssel mit 403.
- **Schlüssel stehen in `Authorization`.** Die Platform API liest nie `x-api-key`.
- **Plattformzugriff wird beim Erstellen eines Schlüssels gewählt**, mit `"access": "platform"` oder `"both"`. Standard ist `inference`, und Plattformzugriff erfordert das Recht `keys.platform_access.grant` ([API-Schlüssel](../console/api-keys.md)).
- **Plattform-Schlüssel entstehen nur in einer Sitzung**, durch jemanden, der jedes Recht der Rolle Admin hat. Sie rufen nie Modelle auf.
- **Schlüssel rotieren mit einer Anfrage.** `POST …/keys/{key}/rotate`, `…/service-accounts/{account}/keys/{key}/rotate` und `orgs/{org}/platform-keys/{key}/rotate` nehmen `grace_hours` (0, 1, 24 oder 168) und den Ablauf des neuen Schlüssels. Sie antworten mit dem neuen Schlüssel, der die Einstellungen des alten behält, und erfordern, was das Erstellen dieses Schlüssels erfordert, sowie das Recht, ihn zu verwalten ([Schlüssel rotieren](../console/api-keys.md#schlüssel-rotieren)).
- **Richtlinien der Organisation gelten für Sitzungen.** Eine Organisation, die Single Sign-on verlangt, beantwortet die Sitzungen der Konten, die sie verwaltet, mit 403, sofern sie nicht von ihrem Anbieter stammen; Sitzungen von Gästen kommen durch. Eine Organisation, die Zwei-Faktor-Authentifizierung verlangt, beantwortet Sitzungen ohne zweiten Faktor mit 403. Ausgenommen ist eine Sitzung aus einem Single Sign-on, das dort zählt: ihrem eigenen oder dem der Organisation, die das Konto verwaltet ([verwaltete Konten](../self-hosting/sign-in.md#verwaltete-konten)).
- **Unbekannte Schlüssel werden gebremst.** Zu viele Anfragen mit unbekannten Schlüsseln von einer Adresse, auf beiden APIs zusammen, erhalten `429 rate_limited` mit `Retry-After` ([Fehlgeschlagene Schlüsselprüfungen](../self-hosting/configuration.md#fehlgeschlagene-schlüsselprüfungen)).

### Websiteübergreifende Anfragen

Eine Anfrage, die das Sitzungs-Cookie trägt und etwas ändert, muss `Origin` mit dem Origin Ihrer Installation senden, etwa `Origin: https://kaeso.example.com`. Dasselbe gilt für jede solche Anfrage an einen öffentlichen `auth/*`-Endpunkt. Browser senden den Header selbst, und `Referer` zählt nicht an seiner Stelle. Fehlt er oder nennt einen anderen Origin, antwortet Kaeso mit `403 permission_denied`, und die Meldung nennt die Adresse, unter der Kaeso zu öffnen ist.

So können andere Websites weder mit Ihrer Sitzung handeln noch Ihren Browser in ein eigenes Konto anmelden.

- Anfragen mit einem API-Schlüssel oder einem Plattform-Schlüssel brauchen kein `Origin`.
- `GET`, `HEAD` und `OPTIONS` brauchen es nie, auch nicht `GET auth/methods`.
- Ein Server, der `POST auth/methods`, `POST auth/password` oder `POST auth/oidc/start` aufruft, sendet den Origin der Installation. Um Personen von einer anderen Website zu Single Sign-on zu schicken, verlinken Sie die Anmeldeseite der Konsole, die zuerst nach ihrer Adresse fragt ([Anmeldung und Single Sign-on](../self-hosting/sign-in.md#die-anmeldeseite)).

### Kürzliche Anmeldung

In einer Sitzung verlangen einige Aktionen eine Anmeldung innerhalb der letzten 15 Minuten. Sonst antwortet Kaeso mit `403 recent_sign_in_required`, und die Konsole bittet Sie, sich erneut anzumelden. Sie bestätigen mit Ihrem Passwort oder einem Anmeldelink, oder über Single Sign-on, wenn das Konto keinen anderen Weg hat. Schlüssel brauchen das nie.

`GET me/sign-in-again` nennt, wie das Konto bestätigen kann: `password`, `magic_link` und `set_password`, wenn es noch kein Passwort hat, aber über `POST auth/password-reset` eines festlegen kann. Nur wenn alle drei `false` sind, nennt `sso` die Organisationen, mit deren Identitätsanbieter das Konto verknüpft ist. Die Organisation, die das Konto verwaltet, steht zuerst. Verlangt diese Single Sign-on, bestätigt nur ihr Anbieter.

`POST me/sign-in-again/sso` startet eine Bestätigung bei einem davon. Kaeso bittet den Anbieter um eine frische Anmeldung (`prompt=login`, `max_age=0`), und nur dasselbe Konto bestätigt. Kaeso ersetzt die Sitzung dann durch eine aus dem Single Sign-on dieser Organisation. Bei einem Fehler kommt der Browser mit `#sso_error=` zu `next` zurück, etwa `other_account` oder `link_required`.

`cannot_confirm` bedeutet, dass die Antwort des Anbieters keine frische Anmeldung zeigte: Sie ließ `auth_time` weg oder trug eine ältere Anmeldung, die der Anbieter wiederverwendet hat. `sso` lässt diese Organisation dann aus, und ihre Verbindung zeigt `confirms_sign_in: false` in `GET orgs/{org}/security`.

Diese Aktionen verlangen eine kürzliche Anmeldung:

- **In Organisationen und Projekten:** die Rechte, die in `orgs/{org}/rights` das Kennzeichen `recent_sign_in` tragen. Das sind `org.delete`, `org.ownership.transfer`, `provider_credentials.create`, `provider_credentials.replace`, `security.sso.manage`, `security.sso.require`, `security.2fa.require`, `platform_keys.create`, `logs.delete` und `keys.platform_access.grant`.
- **Im eigenen Konto:** die E-Mail-Adresse ändern, ein Passwort setzen, wenn das Konto keines hat, die Zwei-Faktor-Authentifizierung abschalten, neue Wiederherstellungscodes erstellen und das Konto löschen.
- **Für Instanz-Admins:** die Kontoerstellung, die Organisationserstellung oder die erlaubten Domains ändern, automatische Updates wählen, ein Update anwenden und eine Organisation zur Löschung vormerken oder löschen. Die Wahl von Single Sign-on für Instanz-Admins verlangt ebenfalls eine, und nur ein Passwort oder ein Anmeldelink bestätigt sie.

## Konventionen

| Thema | Regel |
| --- | --- |
| Format | REST mit JSON: Pfade im Plural in Kebab-Case (`service-accounts`), Felder in `snake_case`. Senden Sie Bodys mit `content-type: application/json`; auf ein Feld, das der Endpunkt nicht kennt, antwortet Kaeso mit 400. |
| IDs | Ein Präfix und ein zeitlich geordneter Teil: `org_…`, `proj_…`, `usr_…`, `mem_…`, `inv_…`, `svc_…`, `key_…`, `opk_…`, `cred_…`, `model_…`, `alias_…`, `lim_…`, `role_…`, `ses_…`, `req_…`, `evt_…`, `exp_…`, `upd_…`. |
| Zeiten | RFC 3339, in UTC. Jede Antwort nennt die Zeitzone der Installation in `Kaeso-Time-Zone`, etwa `Europe/Berlin`, die Zone, in der die Konsole Zeiten zeigt. |
| Geld | US-Dollar als Dezimal-Strings, etwa `"0.0001"`. Senden Sie Preise und Beträge als Dezimal-Strings oder ganze Zahlen; ein JSON-Bruch wie `0.5` erhält 400. |
| Beträge von Limits | Höchstens 10^12 Anfragen oder Token, bei einem Ausgabenbudget 1.000.000.000 USD. |
| Erstellen | `POST` auf die Sammlung, beantwortet mit 201 und der neuen Ressource. |
| Ändern | `PATCH` nur mit den zu ändernden Feldern. |
| Aktionen | `POST …/archive`, `…/disable`, `…/enable`, `…/replace` oder `…/restore`. Archivieren behält den Datensatz. |
| Löschen | `DELETE`, nur wo wirklich etwas gelöscht wird: Anbieter-Schlüssel, Aliasse, Katalogmodelle, Limits, eigene Rollen, Sitzungen, Einladungen und Ihr Konto. |
| Audit | Jede Änderung wird in derselben Transaktion in das Audit-Log ihrer Ebene geschrieben. |
| Anfrage-IDs | Jede Antwort trägt `kaeso-request-id`. |

### Listen

Eine Liste nimmt `limit`, von 1 bis 100 mit dem Standard 20, und `after`, den `next_cursor` der vorigen Seite. Weitere Query-Parameter filtern die Liste. Listen kommen in Erstellungsreihenfolge, außer Anfrageprotokollen und Audit-Logs, die das Neueste zuerst bringen.

```json
{ "data": [], "has_more": true, "next_cursor": "…" }
```

Auf der letzten Seite ist `next_cursor` `null`.

### Revisionen

Katalogmodelle und Limits tragen eine `revision`, die mit jeder Änderung steigt. Lesen, Erstellen oder Ändern antwortet mit `ETag: "<revision>"`. Senden Sie dieses Tag mit `PATCH` oder `DELETE` in `If-Match` zurück, um nur die gelesene Revision zu ändern. Hat sich der Eintrag inzwischen geändert, antwortet Kaeso mit `412 conflict`; ohne `If-Match` gilt die Änderung für die aktuelle Revision.

### Zur Löschung vorgemerkte Organisationen

Eine zur Löschung vorgemerkte Organisation ist schreibgeschützt, bis sie wiederhergestellt oder gelöscht ist. Lesen funktioniert, soweit die Rechte des Aufrufers reichen, und ihre Schlüssel authentifizieren gar nicht. Jede Änderung darin ergibt `409 org_scheduled_for_deletion`, auch sie zu verlassen und eine Einladung in sie anzunehmen. Nur `POST orgs/{org}/restore` und `instance/orgs/{org}/*` der Instanz-Admins funktionieren noch.

`auth/session` kennzeichnet eine solche Organisation in jeder Mitgliedschaft mit `org_state` und `org_deletion_due_at`. Die Rechte darin schrumpfen auf Sehen, Exportieren und `org.delete`. Die Sitzung trägt außerdem die Richtlinie `org_deletion` der Instanz, mit `allowed` und `grace_days`.

## Fehler

Jeder Fehler hat denselben Body:

```json wrap
{
  "error": {
    "type": "permission_denied",
    "message": "You need the right `models.manage` in this project; ask one of its admins for it.",
    "right": "models.manage"
  }
}
```

Die Meldung ist ein kurzer Satz für Menschen, und die Konsole zeigt ihn unverändert. `param` nennt das Feld, den Query- oder Pfadparameter, der den Fehler verursacht. `right` nennt das fehlende Recht, wenn genau dieses eine Recht die Anfrage erlauben würde. Beide fehlen, wo sie nicht zutreffen.

| Status | `type` | Wann |
| --- | --- | --- |
| 400 | `invalid_request` | Der Body ist kein JSON oder hat ein unbekanntes Feld. Ein Parameter hat den falschen Typ oder die falsche Form, oder ein Wert liegt außerhalb des Bereichs. |
| 401 | `authentication` | Keine Sitzung und kein Schlüssel, oder einer, der unbekannt, abgelaufen oder beendet ist. |
| 403 | `permission_denied` | Ein fehlendes Recht, ein Schlüssel auf einer Route, die er nicht nutzen kann, ein fehlendes `Origin` oder eine Richtlinie der Organisation. |
| 403 | `recent_sign_in_required` | Die Aktion verlangt eine [kürzliche Anmeldung](#kürzliche-anmeldung). |
| 403 | `other_sign_in_required` | Die Sitzung stammt aus dem Single Sign-on einer Organisation, die das Konto nicht verwaltet, und die Aktion liegt außerhalb dieser Organisation; oder sie stammt aus Single Sign-on, und die Aktion gehört zur Verwaltung der Instanz, außer aus der Organisation, die für Instanz-Admins gewählt ist. Melden Sie sich mit einem Passwort oder einem Anmeldelink an. |
| 404 | `not_found` | Den Pfad gibt es nicht, oder das Gesuchte gibt es nicht oder der Aufrufer kann es nicht sehen. |
| 405 | `invalid_request` | Der Pfad nimmt diese Methode nicht an. `Allow` nennt die, die er annimmt. |
| 409 | `conflict` | Die Änderung widerspricht dem aktuellen Zustand, etwa ein schon vergebener Name. |
| 409 | `org_scheduled_for_deletion` | Eine Änderung in einer zur Löschung vorgemerkten Organisation. |
| 412 | `conflict` | `If-Match` nennt eine ältere Revision. |
| 413 | `request_too_large` | Der Body ist größer als `KAESO_BODY_LIMITS_PLATFORM_API`, standardmäßig 1 MiB. |
| 429 | `rate_limited` | Zu viele Anmeldeversuche oder unbekannte Schlüssel von einer Adresse, oder ein Limit bei einer Playground-Anfrage. `Retry-After` sagt, wann es wieder geht. |
| 500 | `internal` | Ein Fehler im Gateway. Die Meldung nennt die Anfrage-ID, die Sie melden können. |
| 503 | `unavailable` | Kaeso erreicht eine Datenbank oder einen anderen Dienst nicht, etwa „Kaeso cannot reach its database right now; try again shortly.“ |
| 429, 502, 503, 504 | `provider_error` | Der Anbieter ist gescheitert, nur bei [Playground](#playground)-Anfragen. |

## Routen

Die Routen sind relativ zu `/platform/v1`. [`openapi.json`](#openapi-dokument) nennt jede mit ihren Feldern.

| Pfade | Enthält |
| --- | --- |
| `auth/*` | Einrichtung beim ersten Start, Anmeldemethoden, Anmeldung mit Passwort und zweitem Faktor, Anmeldelinks, Passwort-Reset, Single Sign-on, Sitzungen, Registrierung, wo die Instanz die Adresse zulässt, und Einladungen. `auth/session` liefert den Benutzer, seine Mitgliedschaften mit Rollen und Rechten, die Organisation, die das Konto verwaltet (`managed_by`), und ob er Organisationen erstellen darf (`can_create_orgs`). |
| `me`, `me/*` | Ihr Profil, Bild und Sprache (`locale`: `auto`, `en`, `de` oder `zh-CN`), E-Mail-Adresse, Passwort, Zwei-Faktor-Authentifizierung und Wiederherstellungscodes. Außerdem Ihre Sitzungen, Mitgliedschaften, Schlüssel und Benachrichtigungen, das Erstellen einer Organisation und das Löschen Ihres Kontos. |
| `orgs/{org}/*` | Die Organisation und ihr Bild, Mitglieder, Einladungen, Rollen und Rechte sowie Projekte. Außerdem die Anbieter, die Kaeso aufruft, Anbieter-Schlüssel, Sicherheitseinstellungen, Plattform-Schlüssel, Protokolleinstellungen und Speicher, Nutzung, das Audit-Log, Inhaberwechsel, Löschen und Wiederherstellen. |
| `orgs/{org}/projects/{project}/*` | Das Projekt und sein Bild, Mitglieder mit ihrem Modellzugriff, Dienstkonten, Schlüssel mit ihren Modellen und Limits und die Schlüsselrichtlinie. Außerdem der Katalog und Aliasse, Limits, Inhaltsprotokollierung, Anfrageprotokolle und Exporte, Nutzung, das Audit-Log und der [Playground](#playground). |
| `instance/*` | Für Instanz-Admins: Benutzer, Organisationen, Instanzeinstellungen und die Organisation, deren Single Sign-on die Instanz verwalten darf (`instance/settings/admin-sso`), der Abgleich mit models.dev, Systemzustand und Speicher, Updates und das Audit-Log der Instanz. |

Instanz-Admins sehen die Metadaten jeder Organisation, nie ihre Inhalte. Sie erstellen, benennen um, merken zur Löschung vor, stellen wieder her und löschen Organisationen unter `instance/orgs`. `POST instance/orgs/{org}/owner` macht jeden aktiven Benutzer zum Inhaber, auch den Admin selbst; so gelangt ein Instanz-Admin in eine Organisation, um sie zu verwalten. Das Audit-Log der Organisation hält das fest.

Nur Instanz-Admins setzen die Zwei-Faktor-Authentifizierung eines Benutzers zurück, mit `POST instance/users/{user}/reset-2fa`. Abrechnungsrouten gibt es nicht: Die Rechte `billing.view` und `billing.manage` sind für Kaeso Cloud und tragen in `orgs/{org}/rights` das Kennzeichen `cloud_only`.

`GET orgs/{org}/providers` braucht `org.view` und listet jeden Anbieter, den Kaeso aufruft, in jeder Organisation gleich. Jeder Anbieter hat:

- seine ID und seinen Namen;
- die Seite, auf der Sie seine Schlüssel erstellen (`key_page`);
- womit seine Schlüssel beginnen (`key_prefix`, `null` ohne festes Präfix);
- die Formate, mit denen Kaeso ihn aufruft, das Standardformat zuerst.

[Unterstützte Anbieter](../concepts/supported-providers.md) zeigt dieselbe Liste als Tabelle.

## Rollen und Rechte

Jedes Mitglied einer Organisation hat eine Organisationsrolle, und jedes Projektmitglied und jedes Dienstkonto hat eine Projektrolle. Eine Rolle ist eingebaut oder eine eigene Rolle der Organisation (`role_…`), und Sie können ihr pro Mitglied Rechte hinzufügen oder entziehen. Eine Organisationsrolle hält außerdem Rechte in jedem Projekt. [Rechte](../concepts/rights.md) listet jedes Recht und jede integrierte Rolle.

| Route | Recht | Antwortet mit |
| --- | --- | --- |
| `GET orgs/{org}/rights` | `org.view` | Jedem Recht nach Kategorie, mit Name, Beschreibung, Kennzeichen (`sensitive`, `recent_sign_in`, `cloud_only`, `owner_only`) und den Rechten, die es braucht. |
| `GET orgs/{org}/roles`, `GET …/roles/{role}` | `org.view` | Integrierten und eigenen Rollen mit ihren Rechten und der Zahl ihrer Träger. `?scope=org` oder `?scope=project` filtert sie. |
| `POST orgs/{org}/roles`, `PATCH …/roles/{role}`, `DELETE …/roles/{role}` | `roles.manage` | Erstellt, ändert oder löscht eine eigene Rolle. Eine Organisation hat höchstens 50. |
| `GET orgs/{org}/members/{user}/rights` | `members.view` | Den wirksamen Rechten eines Mitglieds in der Organisation, in jedem Projekt und in jedem Projekt, das der Aufrufer sehen kann, mit ihren Quellen. |
| `GET orgs/{org}/projects/{project}/members/{user}/rights` | `project.members.view` | Den wirksamen Rechten eines Projektmitglieds in diesem Projekt, mit ihren Quellen. |
| `GET orgs/{org}/projects/{project}/my-rights` | `project.view` | Den eigenen Rechten des Aufrufers in diesem Projekt, mit ihren Quellen. |
| `GET orgs/{org}/rights/{right}/grantors` | `members.view` | Bis zu 10 aktiven Mitgliedern, die das Recht haben und Rechte ändern dürfen, Inhaber zuerst. |
| `GET orgs/{org}/projects/{project}/rights/{right}/grantors` | `project.members.view` | Dasselbe, für ein Recht in diesem Projekt. |

Senden Sie eine Vergabe als `{"role": "developer", "rights_added": [], "rights_removed": []}`. `role` ist der Name einer integrierten Rolle oder die ID einer eigenen Rolle; `preset` ist ihr veralteter Name. Die Antwort enthält `role` als `{id, kind, name}`, die wirksamen `rights` und bei Vergaben in der Organisation `project_rights`. Einladungen nehmen `role` und `project_roles: [{project_id, role}]`, die Einrichtung von Single Sign-on nimmt `default_role`.

Jede Änderung von Rechten folgt den [Vergaberegeln](../concepts/rights.md#vergaberegeln):

- Ein Recht zu vergeben, das Sie selbst nicht haben, oder ein Mitglied zu ändern, das ein Recht hat, das Ihnen fehlt, ergibt 403, etwa „You can't grant `audit_log.view`: you don't hold it yourself.“
- Ein Recht ohne eines zu vergeben, das es braucht, oder eines zu entfernen, das ein anderes Recht braucht, ergibt 400, und die Meldung nennt beide. Ein Beispiel ist „`logs.delete` needs `logs.storage.view`; remove `logs.delete` too.“

Ein wirksames Recht nennt seine Quellen:

| Quelle | Das Recht kommt aus |
| --- | --- |
| `owner` | Der Rolle als Inhaber der Organisation |
| `role` | Der Organisationsrolle, für Rechte in der Organisation |
| `project_role` | Der Projektrolle |
| `added` | Einem Recht, das dem Mitglied hinzugefügt wurde |
| `org_role` | Den Rechten der Organisationsrolle in jedem Projekt |
| `org_right` | Einem Recht der Organisation, das in Projekte hineinreicht, etwa `projects.view.all` |
| `key` | Einem API-Schlüssel, der im Projekt handelt |

## Playground

Der [Playground](../console/playground.md) der Konsole sendet Anfragen an die Modelle des Projekts als das angemeldete Mitglied. Beide Routen brauchen eine Sitzung und das Projektrecht `playground.use`. Schlüssel beantworten sie mit 403, denn Apps rufen Modelle auf `/v1` auf.

| Route | Tut |
| --- | --- |
| `GET orgs/{org}/projects/{project}/playground/models` | Listet die Modelle, die das Mitglied aufrufen kann: den Katalog ohne Embeddings-Modelle, eingeschränkt auf den Modellzugriff des Mitglieds. Jedes hat Namen, Aliasse, Aufwandsstufen, Limits und Preise, ob es `temperature` annimmt und ob das Projekt einen Anbieter-Schlüssel dafür hat. |
| `POST orgs/{org}/projects/{project}/playground/responses` | Nimmt eine OpenAI-Responses-Anfrage mit `"stream": true` und antwortet mit dem Event-Stream, den `/v1/responses` sendet, samt Keepalive-Kommentaren. |

Die Anfrage nimmt `model`, `input` (ein String oder Input-Items), `instructions`, `temperature`, `top_p`, `max_output_tokens`, `reasoning.effort`, `reasoning.summary` und `stream`, das `true` sein muss. Mit einer [Sitzung](automation.md#stattdessen-eine-sitzung-verwenden):

```bash
project=https://kaeso.example.com/platform/v1/orgs/org_…/projects/proj_…
curl -N "$project/playground/responses" \
  -b "$JAR" \
  -H "Origin: https://kaeso.example.com" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-haiku-4-5",
    "input": "Write a haiku about gateways.",
    "stream": true
  }'
```

Eine Playground-Anfrage ist eine Modellanfrage wie jede andere, gestellt vom Mitglied ohne Schlüssel:

- **Derselbe Weg.** Das Modell wird im Katalog des Projekts aufgelöst, Aliasse und `provider/model` eingeschlossen, und muss im Modellzugriff des Mitglieds liegen. Kaeso verwendet den Anbieter-Schlüssel des Projekts, und das Schließen des Streams bricht die Anfrage beim Anbieter ab.
- **Limits.** Die Raten und Budgets des Mitglieds und des Projekts gelten. Schlüssel-Limits gelten nicht, da es keinen Schlüssel gibt.
- **Nutzung und Protokolle.** Kaeso berechnet die Anfrage dem Projekt als die des Mitglieds, und die Nutzung nach `key` zeigt sie als `playground`. Ihr Protokolleintrag hat die Quelle `playground` und enthält ihren Inhalt, wenn das Projekt Inhalte protokolliert. `logs` und `logs/export` filtern mit `source=api` oder `source=playground`.

Fehler vor dem Beginn des Streams sind Fehler der Platform API:

| Status | `type` | Ursache |
| --- | --- | --- |
| 400 | `invalid_request` | `param` ist `model` bei einem Modell außerhalb des Katalogs. Bei `"stream": false` ist es `stream`. |
| 404 | `not_found` | Der Anbieter stellt das Modell dem Anbieter-Schlüssel des Projekts nicht bereit, mit der Meldung, die Apps auf `/v1` erhalten ([Modelle, die der Anbieter nicht bereitstellt](errors.md#modelle-die-der-anbieter-nicht-bereitstellt)), oder der Anbieter hat für etwas anderes, das die Anfrage nennt, mit 404 geantwortet. |
| 403 | `permission_denied` | Das Modell liegt außerhalb des Modellzugriffs des Mitglieds. |
| 409 | `conflict` | Das Projekt ist archiviert, oder es hat keinen Anbieter-Schlüssel für den Anbieter des Modells. |
| 409 | `org_scheduled_for_deletion` | Die Organisation ist zur Löschung vorgemerkt. |
| 429 | `rate_limited` | Eine Rate oder ein Budget ist erreicht, und die Meldung nennt sie. |
| 503 | `unavailable` | Kaeso kann den Katalog, die Anbieter-Schlüssel oder die Limits nicht lesen, oder Kaeso startet gerade neu. |
| 429, 502, 503, 504 | `provider_error` | Der Anbieter hat die Anfrage wegen seines Ratenlimits abgelehnt, ist gescheitert oder hat den Anbieter-Schlüssel abgelehnt, ist überlastet oder hat nicht rechtzeitig geantwortet. |

Hat der Stream begonnen, kommen Fehler als Responses-Events vom Typ `error`, wie auf `/v1`.
