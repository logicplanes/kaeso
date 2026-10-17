---
title: Rechte
description: Jedes Recht in einer Organisation und einem Projekt, welche Rollen es haben, und die Regeln für ihre Vergabe.
order: 3
---

Ein Recht erlaubt eine Art von Aktion und wird als `resource.action` geschrieben, etwa `keys.all.view`. Jede Operation der Platform API unter `orgs/{org}` prüft genau ein Recht, auf der Organisation oder dem Projekt in ihrem Pfad. Rollen sind benannte Mengen von Rechten; [Mitglieder und Rollen](../console/members-and-roles.md) zeigt, wie die Konsole sie vergibt.

- Die Instanzverwaltung ist kein Recht, sondern ein Flag an einem Benutzer, und gibt keine Rechte in Organisationen oder Projekten.
- Das eigene Konto braucht keine Rechte.
- Jedes Organisationsrecht erfordert zusätzlich `org.view`, jedes Projektrecht `project.view`. Manche Rechte **erfordern** andere, wie unten angegeben: Eine Änderung, die ein Recht ohne das Erforderliche gibt, wird abgelehnt. Bereits gespeicherte Rechte ändert diese Regel nie.
- **Sensible** Rechte legen sensible Daten offen oder sind schwer rückgängig zu machen; sie mit Bedacht vergeben.
- Rechte mit **kürzlicher Anmeldung** erfordern eine Anmeldung in den letzten 15 Minuten, wenn sie aus einer Konsolensitzung genutzt werden.
- Rechte mit dem Hinweis **nur Kaeso Cloud** wirken auf einer selbst gehosteten Installation nicht.

## Rollen

Jedes Mitglied einer Organisation hat eine Organisationsrolle, und jedes Projektmitglied und Dienstkonto eine Projektrolle. Jedem von ihnen lassen sich über die Rolle hinaus Rechte hinzufügen oder entziehen. Eine Änderung an einer Rolle ändert alle, die sie haben.

**Integrierte Rollen** sind in jeder Organisation gleich und lassen sich nicht ändern. In der Platform API sind ihre IDs ihre englischen Namen in Kleinbuchstaben.

| Organisationsrolle | Beschreibung |
| --- | --- |
| Inhaber | Hat jedes Recht, jetzt und in künftigen Versionen; nur Inhaber verwalten Inhaber. |
| Admin | Verwaltet die Organisation und jedes Projekt, außer Löschen, Inhaberwechsel und Abrechnung. |
| Abrechnung | Verwaltet die Abrechnung und sieht Nutzung und Kosten aller Projekte. |
| Entwickler | Erstellt Projekte und sieht Nutzung, Kosten und Anbieter-Schlüssel; arbeitet in zugewiesenen Projekten. |
| Prüfer | Liest für Prüfungen die ganze Organisation samt Audit-Log und allen Projekten, ändert nichts; sieht nie Prompts oder Ausgaben. |
| Mitglied | Sieht die Organisation und ihre Mitglieder; arbeitet in zugewiesenen Projekten. |

| Projektrolle | Beschreibung |
| --- | --- |
| Admin | Verwaltet das Projekt: Mitglieder, Schlüssel, Dienstkonten, Modelle, Limits, Protokolle und Einstellungen. |
| Entwickler | Ruft Modelle mit eigenen Schlüsseln und im Playground auf und liest das Projekt, auch Prompts und Ausgaben. |
| Betrachter | Liest Einstellungen, Modelle, Limits, Schlüssel, Nutzung und Protokoll-Metadaten des Projekts; ändert nichts. |

### Was jede integrierte Rolle enthält

Eine Organisationsrolle enthält auch **Rechte in jedem Projekt**, zusätzlich zur eigenen Projektrolle des Mitglieds dort.

| Organisationsrolle | Organisationsrechte | Rechte in jedem Projekt | Beginnt im Standardprojekt als |
| --- | --- | --- | --- |
| Inhaber | Alle, auch die künftiger Versionen | Alle | Admin |
| Admin | Alle außer `org.delete`, `org.ownership.transfer` und `billing.manage` | Alle | Admin |
| Abrechnung | `org.view`, `members.view`, `projects.view.all`, `logs.storage.view`, `usage.org.view`, `costs.org.view`, `billing.view`, `billing.manage` | Keine | Betrachter |
| Entwickler | `org.view`, `members.view`, `projects.create`, `provider_credentials.view`, `logs.storage.view`, `usage.org.view`, `costs.org.view` | Keine | Entwickler |
| Prüfer | `org.view`, `members.view`, `projects.view.all`, `provider_credentials.view`, `platform_keys.view`, `logs.storage.view`, `usage.org.view`, `costs.org.view`, `billing.view`, `audit_log.view`, `audit_log.export` | Die des Betrachters und `project.audit_log.view` | Betrachter |
| Mitglied | `org.view`, `members.view` | Keine | Betrachter |

| Projektrolle | Rechte |
| --- | --- |
| Admin | Alle Projektrechte |
| Entwickler | Die des Betrachters und `api.request`, `playground.use`, `keys.own.create`, `keys.own.manage`, `logs.content.view` |
| Betrachter | `project.view`, `project.members.view`, `service_accounts.view`, `keys.all.view`, `models.view`, `limits.view`, `logs.metadata.view`, `usage.view`, `costs.view` |

Zwei Organisationsrechte reichen ebenfalls in jedes Projekt: `projects.view.all` gibt `project.view`, und `projects.archive.any` gibt `project.view` und `project.archive`. Die Rechte eines Mitglieds in einem Projekt ergeben sich aus seiner Projektrolle, den Rechten seiner Organisationsrolle in jedem Projekt und dem, was diese beiden Rechte erreichen.

Im Standardprojekt beginnen Inhaber und Admins als Projekt-Admin, Entwickler als Entwickler und jede andere Organisationsrolle, eigene Rollen eingeschlossen, als Betrachter.

### Eigene Rollen

- Jede Organisation kann mit `roles.manage` bis zu 50 eigene Rollen festlegen, für die Organisation oder für Projekte. Organisationsrollen enthalten Organisationsrechte und Rechte in jedem Projekt; Projektrollen enthalten Projektrechte. Alle mit `org.view` sehen sie.
- Ein Name hat 1 bis 50 Zeichen, ist pro Organisation und Geltungsbereich ohne Rücksicht auf Groß- und Kleinschreibung eindeutig und weder der Name einer integrierten Rolle dieses Geltungsbereichs noch `custom`; eine Beschreibung hat höchstens 200 Zeichen.
- Eigene Rollen können keine Inhaber machen: Inhaber ist nur die integrierte Rolle.
- Eine Rolle, die ein Mitglied, ein aktives Dienstkonto oder eine offene Einladung innehat, lässt sich nicht löschen, ebenso wenig die Rolle für Personen, die über Single Sign-on kommen.

## Vergaberegeln

1. **Nur vergeben, was man selbst hat.** Rolle oder Rechte eines Mitglieds ändern, einladen, ein Projektmitglied hinzufügen, ein Dienstkonto erstellen oder ändern, eine eigene Rolle erstellen, ändern oder löschen und die Rolle für Personen wählen, die über Single Sign-on kommen, darf nur Rechte geben, die der Aufrufer hat: Organisationsrechte in der Organisation, Projektrechte in diesem Projekt und Rechte in jedem Projekt nur, wenn der Aufrufer sie in jedem Projekt hat.
2. **Nur auf Gleichgestellte wirken.** Rolle und Rechte eines Mitglieds ändert, oder das Mitglied entfernt, nur jemand, der jedes Recht hat, das das Mitglied in diesem Geltungsbereich hat.
3. **Inhaber** werden nur von Inhabern gemacht, geändert, entfernt und eingeladen. Eine Organisation verliert nie ihren letzten Inhaber.
4. **Erforderliche Rechte** werden für jedes Recht geprüft, das eine Änderung gibt. Eine Änderung kann kein Recht entfernen, das ein verbleibendes Recht erfordert, auch wenn diesem schon ein anderes seiner erforderlichen Rechte fehlt. Eine Änderung einer eigenen Rolle wird abgelehnt, wenn danach jemand, der die Rolle hat, ein Recht – ihm hinzugefügt oder von der Rolle gegeben – ohne ein Recht hätte, das es erfordert; die eigenen Rechte der Personen ändern sich dabei nie.
5. **Schlüssel haben keine eigenen Rechte.** Ein API-Schlüssel handelt als sein Mitglied oder Dienstkonto in seinem Projekt, ein Plattform-Schlüssel einer Organisation mit den Rechten der Rolle Admin in seiner Organisation.

Die [Platform API](../api/platform-api.md) beschreibt die Routen für Rechte, Rollen und Vergaben.

## Organisationsrechte

### Organisation

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `org.view` | Organisation ansehen | Die Organisation, ihre Einstellungen und Rollen sehen. |  |
| `org.update` | Organisation bearbeiten | Die Organisation umbenennen, Bild und allgemeine Einstellungen ändern und das Standardprojekt wählen. |  |
| `org.delete` | Organisation löschen | Die Organisation zur Löschung vormerken oder sie in der Wartezeit wiederherstellen. | sensibel; kürzliche Anmeldung |
| `org.ownership.transfer` | Inhaberschaft übertragen | Ein anderes Mitglied an Ihrer Stelle zum Inhaber machen. | sensibel; kürzliche Anmeldung; erfordert `members.view` |

### Mitglieder & Zugriff

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `members.view` | Mitglieder ansehen | Mitglieder, ihre Rollen und ihre Rechte sehen. |  |
| `members.invite` | Mitglieder einladen | Personen per E-Mail mit einer Rolle einladen, die Sie selbst vergeben dürften. | erfordert `members.view` |
| `members.invites.revoke` | Einladungen zurückziehen | Offene Einladungen zurückziehen. | erfordert `members.view` |
| `members.remove` | Mitglieder entfernen | Mitglieder aus der Organisation entfernen und ihre Schlüssel darin archivieren. | sensibel; erfordert `members.view` |
| `members.rights.update` | Rollen zuweisen | Organisationsrollen und einzelne Organisationsrechte von Mitgliedern ändern. | erfordert `members.view` |
| `roles.manage` | Rollen verwalten | Eigene Rollen der Organisation erstellen, bearbeiten und löschen. |  |

### Projekte

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `projects.view.all` | Alle Projekte ansehen | Jedes Projekt sehen, auch solche ohne das Mitglied. |  |
| `projects.create` | Projekte erstellen | Projekte erstellen; wer eines erstellt, wird dessen Admin. |  |
| `projects.archive.any` | Beliebige Projekte archivieren | Jedes Projekt samt allen Schlüsseln archivieren. | sensibel; erfordert `projects.view.all` |

### Anbieter

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `provider_credentials.view` | Anbieter-Schlüssel ansehen | Die Anbieter-Schlüssel der Organisation sehen (nur Metadaten, nie Secrets). |  |
| `provider_credentials.create` | Anbieter-Schlüssel hinzufügen | Der Organisation Anbieter-Schlüssel hinzufügen. | kürzliche Anmeldung; erfordert `provider_credentials.view` |
| `provider_credentials.replace` | Anbieter-Schlüssel ändern | Anbieter-Schlüssel ersetzen und umbenennen, je Anbieter den Standard wählen und Schlüssel Projekten zuweisen. | kürzliche Anmeldung; erfordert `provider_credentials.view` |
| `provider_credentials.delete` | Anbieter-Schlüssel löschen | Einen Anbieter-Schlüssel aus der Organisation entfernen. | sensibel; erfordert `provider_credentials.view` |

### Sicherheit

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `security.sso.manage` | SSO verwalten | Den Identitätsanbieter der Organisation einrichten und die Rolle der Personen, die darüber kommen. | kürzliche Anmeldung |
| `security.sso.require` | SSO verlangen | Von Mitgliedern die Anmeldung bei dieser Organisation per SSO verlangen. | sensibel; kürzliche Anmeldung |
| `security.2fa.require` | Zwei-Faktor verlangen | Zwei-Faktor-Authentifizierung für Mitglieder verlangen. | kürzliche Anmeldung |
| `security.key_policy.manage` | Schlüssellaufzeit festlegen | Die maximale Schlüssellaufzeit der Organisation festlegen. |  |

### Schlüssel

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `platform_keys.view` | Plattform-Schlüssel ansehen | Die Plattform-Schlüssel der Organisation sehen (Metadaten). |  |
| `platform_keys.create` | Plattform-Schlüssel erstellen | Plattform-Schlüssel erstellen, die alles auf Organisationsebene verwalten. | sensibel; kürzliche Anmeldung; erfordert `platform_keys.view` |
| `platform_keys.revoke` | Plattform-Schlüssel deaktivieren | Plattform-Schlüssel der Organisation deaktivieren, aktivieren oder archivieren. | erfordert `platform_keys.view` |

### Protokolle & Daten

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `logs.settings.manage` | Protokollierung verwalten | Aufbewahrung festlegen und Inhaltsprotokollierung der Organisation ein- oder ausschalten. |  |
| `logs.storage.view` | Protokollspeicher ansehen | Sehen, wie viele Protokolleinträge es gibt und wie viel Platz sie belegen. |  |
| `logs.delete` | Protokolle löschen | Protokolle der Organisation für einen Zeitraum löschen. | sensibel; kürzliche Anmeldung; erfordert `logs.storage.view` |

### Nutzung & Abrechnung

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `usage.org.view` | Gesamtnutzung ansehen | Die Nutzung aller Projekte sehen. |  |
| `costs.org.view` | Gesamtkosten ansehen | Die Kosten aller Projekte sehen. | erfordert `usage.org.view` |
| `billing.view` | Abrechnung ansehen | Tarif, Rechnungen und Zahlungsstatus sehen (Kaeso Cloud). | nur Kaeso Cloud |
| `billing.manage` | Abrechnung verwalten | Tarif und Zahlungsmethode ändern (Kaeso Cloud). | nur Kaeso Cloud; erfordert `billing.view` |

### Audit

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `audit_log.view` | Audit-Log ansehen | Das Audit-Log der Organisation lesen. |  |
| `audit_log.export` | Audit-Log exportieren | Das Audit-Log der Organisation exportieren. | erfordert `audit_log.view` |

## Projektrechte

### Projekt

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `project.view` | Projekt ansehen | Das Projekt und seine Einstellungen sehen. |  |
| `project.update` | Projekt bearbeiten | Das Projekt umbenennen und seine Einstellungen und sein Bild ändern. |  |
| `project.archive` | Projekt archivieren | Dieses Projekt samt allen Schlüsseln archivieren. | sensibel |

### Mitglieder & Zugriff

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `project.members.view` | Mitglieder ansehen | Die Mitglieder des Projekts, ihre Rollen und ihre Rechte sehen. |  |
| `project.members.add` | Mitglieder hinzufügen | Mitglieder der Organisation mit einer Rolle hinzufügen, die Sie selbst vergeben dürften. | erfordert `project.members.view` |
| `project.members.remove` | Mitglieder entfernen | Mitglieder aus dem Projekt entfernen und ihre Schlüssel darin archivieren. | erfordert `project.members.view` |
| `project.members.rights.update` | Rollen zuweisen | Projektrollen und einzelne Projektrechte von Mitgliedern ändern. | erfordert `project.members.view` |
| `service_accounts.view` | Dienstkonten ansehen | Die Dienstkonten des Projekts und ihre Rechte sehen. |  |
| `service_accounts.create` | Dienstkonten erstellen | Dienstkonten mit Rechten erstellen, die Sie selbst haben. | erfordert `service_accounts.view` |
| `service_accounts.rights.update` | Dienstkonten ändern | Dienstkonten umbenennen und ihre Rollen und Rechte ändern. | erfordert `service_accounts.view` |
| `service_accounts.archive` | Dienstkonten archivieren | Dienstkonten samt ihren Schlüsseln archivieren. | erfordert `service_accounts.view` |

### Modellnutzung

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `api.request` | Modelle mit Schlüsseln aufrufen | Die Modelle des Projekts mit eigenen API-Schlüsseln aufrufen. |  |
| `playground.use` | Playground nutzen | Die Modelle des Projekts im Playground der Konsole aufrufen. |  |

### Schlüssel

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `keys.own.create` | Eigene Schlüssel erstellen | API-Schlüssel für sich selbst erstellen. |  |
| `keys.own.manage` | Eigene Schlüssel verwalten | Eigene API-Schlüssel deaktivieren, aktivieren oder archivieren. |  |
| `keys.all.view` | Alle Schlüssel ansehen | Jeden Schlüssel im Projekt sehen (Metadaten). |  |
| `keys.all.manage` | Alle Schlüssel verwalten | Schlüssel aller Personen im Projekt deaktivieren, aktivieren oder archivieren. | erfordert `keys.all.view` |
| `service_accounts.keys.create` | Dienstkonto-Schlüssel erstellen | Schlüssel für die Dienstkonten des Projekts erstellen. | erfordert `service_accounts.view` |
| `keys.platform_access.grant` | Plattformzugriff vergeben | Einem neuen Schlüssel Zugriff auf die Platform API geben. | sensibel; kürzliche Anmeldung |
| `security.key_policy.project.manage` | Schlüssellaufzeit festlegen | Die maximale Schlüssellaufzeit des Projekts festlegen. |  |

### Modelle

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `models.view` | Modelle ansehen | Modelle, Aliasse und Routing des Projekts sehen. |  |
| `models.manage` | Modelle verwalten | Modelle und Aliasse hinzufügen, ändern und entfernen. | erfordert `models.view` |

### Limits & Budgets

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `limits.view` | Limits ansehen | Projekt-, Mitglieds- und Schlüssellimits, Budgets und Modellzugriff sehen. |  |
| `limits.manage` | Limits verwalten | Projekt-, Mitglieds- und Schlüssellimits, Budgets und Modellzugriff ändern. | erfordert `limits.view` |

### Protokolle & Daten

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `logs.metadata.view` | Anfrageprotokolle ansehen | Anfrageprotokolle sehen: Modell, Token, Kosten, Latenz, Status und Schlüssel. |  |
| `logs.content.view` | Prompts und Ausgaben ansehen | Anfrage- und Antwortinhalte in den Protokollen sehen. | sensibel; erfordert `logs.metadata.view` |
| `logs.export` | Protokolle exportieren | Die Anfrageprotokolle des Projekts exportieren. | sensibel; erfordert `logs.metadata.view` |
| `logs.content.disable` | Inhaltsprotokollierung ausschalten | Die Inhaltsprotokollierung für dieses Projekt ausschalten. |  |

### Nutzung & Kosten

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `usage.view` | Nutzung ansehen | Die Nutzung des Projekts sehen. |  |
| `costs.view` | Kosten ansehen | Die Kosten des Projekts sehen. | erfordert `usage.view` |

### Audit

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `project.audit_log.view` | Projekt-Audit-Log ansehen | Das Audit-Log der Änderungen in diesem Projekt lesen. |  |
