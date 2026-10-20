---
title: Rechte
description: Jedes Recht in einer Organisation und einem Projekt nachschlagen, welche Rollen es haben und nach welchen Regeln es vergeben wird.
order: 3
---

Ein Recht erlaubt eine Art von Aktion und wird als `resource.action` geschrieben, etwa `keys.all.view`. Jede Operation der Platform API unter `orgs/{org}` prüft genau ein Recht, auf der Organisation oder dem Projekt in ihrem Pfad. [Mitglieder und Rollen](../console/members-and-roles.md) zeigt, wie Sie Rollen und Rechte in der Konsole vergeben.

Ihr eigenes Konto zu verwalten, braucht kein Recht. Auch die Instanzverwaltung ist kein Recht: Sie ist ein Flag an einem Benutzer und gibt keine Rechte in Organisationen oder Projekten.

## Hinweise

Die Tabellen unten kennzeichnen manche Rechte mit diesen Hinweisen:

- **Erfordert.** Das Recht lässt sich nur zusammen mit den Rechten vergeben, die es nennt. Jedes Organisationsrecht erfordert zusätzlich `org.view`, jedes Projektrecht `project.view`. Kaeso lehnt eine Änderung ab, die ein Recht ohne das Erforderliche gibt, ändert aber nie bereits gespeicherte Rechte.
- **Sensibel.** Das Recht legt sensible Daten offen oder ist schwer rückgängig zu machen. Vergeben Sie es mit Bedacht.
- **Kürzliche Anmeldung.** Wer das Recht aus einer Konsolensitzung nutzt, muss sich in den letzten 15 Minuten angemeldet haben.
- **Nur Kaeso Cloud.** Das Recht wirkt auf einer selbst gehosteten Installation nicht.

## Rollen

Jedes Mitglied einer Organisation hat eine Organisationsrolle. Jedes Projektmitglied und jedes Dienstkonto hat eine Projektrolle. Über die Rolle hinaus können Sie einzelne Rechte hinzufügen oder entziehen. Eine Änderung an einer Rolle gilt für alle, die sie haben.

Integrierte Rollen sind in jeder Organisation gleich, und niemand kann sie ändern. In der Platform API sind ihre IDs ihre englischen Namen in Kleinbuchstaben.

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

Eine Organisationsrolle enthält auch Rechte in jedem Projekt, zusätzlich zur eigenen Projektrolle des Mitglieds dort:

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

Zwei Organisationsrechte reichen ebenfalls in jedes Projekt. `projects.view.all` gibt `project.view`, und `projects.archive.any` gibt `project.view` und `project.archive`.

Die Rechte eines Mitglieds in einem Projekt ergeben sich aus drei Quellen: seiner Projektrolle, den Rechten seiner Organisationsrolle in jedem Projekt und dem, was diese beiden Rechte erreichen. Im Standardprojekt beginnen Inhaber und Admins als Projekt-Admin und Entwickler als Entwickler. Jede andere Organisationsrolle beginnt als Betrachter, eigene Rollen eingeschlossen.

### Eigene Rollen

- Mit `roles.manage` kann eine Organisation bis zu 50 eigene Rollen festlegen, für die Organisation oder für Projekte. Organisationsrollen enthalten Organisationsrechte und Rechte in jedem Projekt. Projektrollen enthalten Projektrechte.
- Alle mit `org.view` sehen die eigenen Rollen.
- Ein Name hat 1 bis 50 Zeichen und ist pro Organisation und Geltungsbereich eindeutig, ohne Rücksicht auf Groß- und Kleinschreibung. Er darf weder `custom` noch der Name einer integrierten Rolle dieses Geltungsbereichs sein. Eine Beschreibung hat höchstens 200 Zeichen.
- Eigene Rollen können keine Inhaber machen. Inhaber ist nur die integrierte Rolle, und nur Inhaber haben `org.ownership.transfer`: Keine eigene Rolle und keine einzelne Änderung kann es vergeben.
- Eine Rolle, die ein Mitglied, ein aktives Dienstkonto oder eine offene Einladung hat, können Sie nicht löschen. Ebenso wenig die Rolle, die Personen erhalten, die über Single Sign-on kommen.

## Vergaberegeln

Jede Änderung an Rollen und Rechten folgt diesen Regeln:

1. **Nur vergeben, was Sie selbst haben.** Sie können nur Rechte vergeben, die Sie selbst im selben Bereich haben: Organisationsrechte in der Organisation, Projektrechte in diesem Projekt. Rechte in jedem Projekt können Sie nur vergeben, wenn Sie sie in jedem Projekt haben. Das gilt, wenn Sie Rolle oder Rechte eines Mitglieds ändern, einladen und eine Einladung erneut senden, ein Projektmitglied hinzufügen, ein Dienstkonto oder eine eigene Rolle erstellen oder ändern und die Rolle für Single Sign-on wählen.
2. **Nur auf Gleichgestellte wirken.** Sie können Rolle und Rechte eines Mitglieds nur ändern oder das Mitglied entfernen, wenn Sie jedes Recht haben, das das Mitglied in diesem Geltungsbereich hat. Dasselbe gilt, wenn Sie ein Dienstkonto ändern oder archivieren.
3. **Inhaber verwalten Inhaber.** Nur Inhaber machen, ändern, entfernen und laden Inhaber ein, und nur ein Inhaber überträgt die Inhaberschaft. Eine Organisation verliert nie ihren letzten Inhaber.
4. **Erforderliche Rechte werden geprüft.** Kaeso prüft, was jedes Recht erfordert, das eine Änderung gibt. Eine Änderung kann kein Recht entfernen, das ein verbleibendes Recht erfordert, auch wenn diesem schon ein anderes seiner erforderlichen Rechte fehlt. Kaeso lehnt eine Änderung an einer eigenen Rolle ab, nach der ein Träger der Rolle ein Recht ohne ein erforderliches hätte. Die eigenen Rechte der Träger ändern sich dabei nie.
5. **Schlüssel haben keine eigenen Rechte.** Ein API-Schlüssel handelt als sein Mitglied oder Dienstkonto in seinem Projekt. Ein Plattform-Schlüssel einer Organisation handelt mit den Rechten der Rolle Admin in seiner Organisation.

Die [Platform API](../api/platform-api.md) beschreibt die Routen für Rechte, Rollen und Vergaben.

## Organisationsrechte

### Organisation

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `org.view` | Organisation ansehen | Die Organisation, ihre Einstellungen und Rollen sehen. |  |
| `org.update` | Organisation bearbeiten | Die Organisation umbenennen, Bild und allgemeine Einstellungen ändern und das Standardprojekt wählen. |  |
| `org.delete` | Organisation löschen | Die Organisation zur Löschung vormerken oder sie in der Wartezeit wiederherstellen. | sensibel; kürzliche Anmeldung |
| `org.ownership.transfer` | Inhaberschaft übertragen | Ein anderes Mitglied an Ihrer Stelle zum Inhaber machen; nur Inhaber haben dieses Recht. | sensibel; kürzliche Anmeldung; erfordert `members.view` |

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
| `security.sso.manage` | SSO verwalten | Den Identitätsanbieter der Organisation einrichten und die Rolle der Personen festlegen, die darüber beitreten. | kürzliche Anmeldung |
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
| `logs.content.manage` | Inhaltsprotokollierung schalten | Die Inhaltsprotokollierung für dieses Projekt ein- oder ausschalten. | sensibel |

### Nutzung & Kosten

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `usage.view` | Nutzung ansehen | Die Nutzung des Projekts sehen. |  |
| `costs.view` | Kosten ansehen | Die Kosten des Projekts sehen. | erfordert `usage.view` |

### Audit

| Recht | Name | Was es erlaubt | Hinweise |
| --- | --- | --- | --- |
| `project.audit_log.view` | Projekt-Audit-Log ansehen | Das Audit-Log der Änderungen in diesem Projekt lesen. |  |
