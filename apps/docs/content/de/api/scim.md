---
title: SCIM-Bereitstellung
description: Lassen Sie Ihren Identitätsanbieter Mitglieder über SCIM 2.0 hinzufügen und entfernen, mit archivierten Schlüsseln, wenn sie gehen.
order: 13
---

Eine Organisation mit Single Sign-on kann ihren Identitätsanbieter Mitglieder über SCIM 2.0 hinzufügen und entfernen lassen ([RFC 7644](https://datatracker.ietf.org/doc/html/rfc7644), Schemas in [RFC 7643](https://datatracker.ietf.org/doc/html/rfc7643)). Verlässt jemand das Unternehmen und deaktiviert der Anbieter ihn, entfernt Kaeso ihn aus der Organisation und ihren Projekten und archiviert dort seine API-Schlüssel, ohne dass jemand die Konsole öffnet.

## Einrichten

1. Verbinden Sie Single Sign-on und bestätigen Sie Ihre E-Mail-Domains ([Single Sign-on](../console/organizations-and-projects.md#single-sign-on)). Wählen Sie unter **Beitritt ohne Einladung möglich?** die Rolle, mit der Personen beitreten: Bereitgestellte Mitglieder bekommen sie.
2. Wählen Sie unter **Sicherheit** › **Bereitstellung** **Token erstellen** und kopieren Sie es. Es wird einmal gezeigt, und Kaeso speichert nur seinen Hash. Dafür braucht es das Recht **SSO verwalten** und eine kürzliche Anmeldung; Agenten und Plattform-Schlüssel können das nicht.
3. Geben Sie der App bei Ihrem Identitätsanbieter die **SCIM-Basis-URL** aus demselben Abschnitt, `https://kaeso.example.com/platform/v1/orgs/{org}/scim/v2`, und das Token als HTTP-Bearer-Token.

**Okta**: Wählen Sie im Reiter **Provisioning** der App **SCIM 2.0**, geben Sie die Basis-URL ein, setzen Sie **Unique identifier field for users** auf `userName` und die Authentifizierung auf **HTTP Header**, und schalten Sie **Push New Users**, **Push Profile Updates** und **Push Groups** ein ([SCIM-Leitfaden von Okta](https://developer.okta.com/docs/api/openapi/okta-scim/guides/scim-20)).

**Microsoft Entra ID**: Setzen Sie unter **Provisioning** der Unternehmensanwendung den Modus auf **Automatic**, geben Sie die Basis-URL als **Tenant URL** und das Token als **Secret Token** ein, dann **Test Connection** ([SCIM-Leitfaden von Entra ID](https://learn.microsoft.com/en-us/entra/identity/app-provisioning/use-scim-to-provision-users-and-groups)).

**Token ersetzen** erstellt ein neues; das ersetzte gilt noch 24 Stunden, damit Sie den Anbieter umstellen können. **Löschen** beendet die Bereitstellung sofort; Mitglieder bleiben.

## Was es tut

SCIM verwaltet nur Konten an den bestätigten Domains der Organisation. Ein Gast, dessen Adresse an einer anderen Domain liegt, wird nie gelistet, hinzugefügt oder entfernt. Dienstkonten bleiben unberührt.

| Der Anbieter | Kaeso |
| --- | --- |
| Legt einen Benutzer an | Legt das Konto an, wenn es keines gibt, und fügt die Mitgliedschaft mit der Standardrolle von Single Sign-on hinzu. Die erste Anmeldung der Person über den Anbieter verknüpft ihre Identität. Ein bestehendes Konto an einer bestätigten Domain tritt ebenfalls bei |
| Ändert den Namen eines Benutzers | Ändert den Namen des Kontos. Die Adresse lässt sich über SCIM nicht ändern |
| Setzt `active` auf false | Entfernt das Mitglied aus der Organisation und ihren Projekten und archiviert dort seine Schlüssel, wie **Aus Organisation entfernen**, und beendet die Sitzungen, die der Anbieter der Organisation angemeldet hat, samt den darin freigegebenen Agenten und Portal-Clients. Ist die Person in keiner Organisation mehr, enden alle ihre Sitzungen |
| Setzt `active` wieder auf true | Fügt eine neue Mitgliedschaft mit der Standardrolle hinzu. Archivierte Schlüssel bleiben archiviert |
| Löscht einen Benutzer | Entfernt das Mitglied wie oben und vergisst, dass der Anbieter es verwaltet |
| Überträgt Gruppen | Speichert die Gruppen und ihre Mitglieder. Sie gewähren noch keinen Zugriff |

Der letzte Owner der Organisation wird nie entfernt: Der Anbieter bekommt `409` mit einem `detail`, das es sagt. Das Audit-Log verzeichnet `member.provision` und `member.deprovision` mit der `externalId` des Benutzers, und die E-Mail **Mitgliederänderungen** geht wie bei anderen Mitgliedern hinaus.

**Sicherheit** › **Bereitstellung** zeigt, wann der Anbieter zuletzt anfragte, wie viele Mitglieder er aktiv hält und die letzte Anfrage, die Kaeso ablehnte, mit ihrem Grund.

## Endpunkte

Jede Anfrage trägt das Token der Organisation als `Authorization: Bearer …`. Ein falsches Token oder das einer anderen Organisation bekommt `401`, und wiederholte Fehlschläge von einer Adresse werden gedrosselt wie unbekannte API-Schlüssel. Solange die Organisation keine Verbindung für Single Sign-on hat, bekommt jede Anfrage `403`. Antworten sind `application/scim+json`; Fehler folgen RFC 7644, etwa `{"schemas": ["urn:ietf:params:scim:api:messages:2.0:Error"], "status": "409", "scimType": "uniqueness", "detail": "…"}`.

| Methode und Pfad | Was er tut |
| --- | --- |
| `GET /ServiceProviderConfig`, `GET /ResourceTypes`, `GET /Schemas` | Was Kaeso unterstützt: `PATCH` und Filter, kein Bulk, keine Sortierung, keine ETags |
| `GET /Users` | Listet die Mitglieder an bestätigten Domains und die Benutzer, die der Anbieter deaktiviert hat. Filter `userName eq "…"` und `externalId eq "…"`; `startIndex` und `count`, höchstens 200 |
| `POST /Users` | Legt einen Benutzer an: `userName` (die E-Mail-Adresse), `externalId`, `name`, `displayName`, `emails`, `active` |
| `GET`, `PUT`, `PATCH`, `DELETE /Users/{id}` | Liest, ersetzt, ändert oder löscht einen Benutzer. Die ID ist die Benutzer-ID von Kaeso, etwa `usr_…` |
| `GET /Groups`, `POST /Groups` | Listet oder legt Gruppen an; Filter `displayName eq` und `externalId eq` |
| `GET`, `PUT`, `PATCH`, `DELETE /Groups/{id}` | Liest, ersetzt, ändert oder löscht eine Gruppe |

`PATCH` nimmt Operationen so, wie die Anbieter sie senden: `op` in beliebiger Schreibweise, mit oder ohne `path`, und `active` als Boolean oder als Zeichenkette `"True"` oder `"False"`. Attribute, von denen Kaeso nichts speichert, etwa ein Titel oder die Enterprise-Erweiterung, werden angenommen und weggelassen.

Die Platform-API verwaltet das Token: `GET /orgs/{org}/scim`, `POST /orgs/{org}/scim/token` und `DELETE /orgs/{org}/scim/token` ([Platform-API](platform-api.md)).
