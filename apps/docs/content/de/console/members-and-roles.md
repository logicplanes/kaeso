---
title: Mitglieder und Rollen
description: Personen einladen, ihnen Rollen und Rechte geben und Dienstkonten erstellen.
order: 2
---

Jede Person in einer Organisation hat eine **Organisationsrolle** und in jedem Projekt, dem sie angehört, eine **Projektrolle**. Eine Rolle ist eine Menge von Rechten; einem Mitglied lassen sich auch einzelne Rechte hinzufügen oder entziehen. Die [Referenz der Rechte](../concepts/rights.md) listet jedes Recht.

## Rollen

| Organisationsrolle | Was sie erlaubt |
| --- | --- |
| Inhaber | Jedes Recht, jetzt und in künftigen Versionen. Nur Inhaber verwalten Inhaber, und eine Organisation behält mindestens einen. |
| Admin | Verwaltet die Organisation und jedes Projekt, außer die Organisation zu löschen, die Inhaberschaft zu übertragen und die Abrechnung zu ändern |
| Abrechnung | Die Abrechnung sowie Nutzung und Kosten aller Projekte |
| Entwickler | Erstellt Projekte und sieht Nutzung, Kosten und Anbieter-Schlüssel; arbeitet in den Projekten, denen er hinzugefügt wurde |
| Prüfer | Liest für Prüfungen die ganze Organisation, samt Audit-Log und jedem Projekt, ohne etwas zu ändern oder Prompts und Ausgaben zu sehen |
| Mitglied | Sieht die Organisation und ihre Mitglieder; arbeitet in den Projekten, denen es hinzugefügt wurde |

| Projektrolle | Was sie erlaubt |
| --- | --- |
| Admin | Verwaltet das Projekt: Mitglieder, Schlüssel, Dienstkonten, Modelle, Limits, Protokolle und Einstellungen |
| Entwickler | Ruft Modelle mit eigenen Schlüsseln und im Playground auf und liest das Projekt, auch Prompts und Ausgaben |
| Betrachter | Liest Einstellungen, Modelle, Limits, Schlüssel, Nutzung und Protokoll-Metadaten des Projekts, ohne etwas zu ändern |

Inhaber und Admins haben jedes Recht in jedem Projekt. Im Standardprojekt treten Inhaber und Admins als Admin bei, Entwickler als Entwickler und alle anderen als Betrachter.

### Eigene Rollen

Unter **Organisationseinstellungen** › **Mitglieder** › **Rollen** legt **Rolle erstellen** eine eigene Rolle für die Organisation oder für Projekte an, ausgehend von einer integrierten Rolle oder ohne Rechte. Eine Organisationsrolle kann auch Rechte in jedem Projekt enthalten. Integrierte Rollen lassen sich nicht ändern; mit **Duplizieren** dient eine als Ausgangspunkt. Eine Organisation hat höchstens 50 eigene Rollen, und eine Rolle, die jemand innehat, lässt sich nicht löschen.

Vergeben lassen sich nur Rechte, die man selbst hat, und ändern lassen sich nur Mitglieder, deren Rechte man alle selbst hat.

## Personen einladen

**Mitglied einladen** auf der Seite **Mitglieder** der Organisation sendet eine Einladung per E-Mail:

1. Eine oder mehrere E-Mail-Adressen eingeben.
2. Ihre Rolle in der Organisation wählen.
3. Die Projekte hinzufügen, denen sie beitreten, jeweils mit einer Rolle. Dem Standardprojekt treten sie immer bei.

Eine Einladung gilt 21 Tage. **Einladungen** listet sie, mit **Erneut senden** und **Widerrufen**. Organisationen mit Single Sign-on können Personen aus ihren E-Mail-Domains auch ohne Einladung beitreten lassen.

## Das Sheet eines Mitglieds

Ein Mitglied wählen, um sein Sheet zu öffnen: seine Rolle, mit **Anpassen**, um einzelne Rechte hinzuzufügen oder zu entziehen; jedes Recht, das es hat, und woher es kommt; seine Projekte; und **Aus der Organisation entfernen**. Ein Inhaber kann ihm mit **Inhaberschaft übertragen** die Inhaberschaft übergeben und wird selbst Admin.

Wer aus einer Organisation oder einem Projekt entfernt wird, dessen Schlüssel dort werden archiviert.

## Projektmitglieder

Auf der Seite **Mitglieder** eines Projekts fügt **Mitglied hinzufügen** jemanden mit einer Projektrolle hinzu, der schon in der Organisation ist; andere zuerst in die Organisation einladen. Das Sheet eines Projektmitglieds legt auch fest, welche Modelle es aufrufen darf und welche [Limits](limits-and-budgets.md) gelten, und **Aus Projekt entfernen** nimmt es aus dem Projekt, während es in der Organisation bleibt.

## Dienstkonten

Ein **Dienstkonto** ist ein Mitglied für Software statt für eine Person: ein Backend, ein Job, eine Pipeline. Seine Schlüssel hängen nicht davon ab, dass jemand in der Organisation bleibt.

Auf der Seite **Dienstkonten** eines Projekts fragt **Dienstkonto erstellen**, was es nutzt, etwa `checkout-production`, und nach seiner Projektrolle. Sein Sheet enthält seine Schlüssel mit **Schlüssel erstellen**, seinen Modellzugriff und seine Limits. **Dienstkonto archivieren** archiviert es samt Schlüsseln, endgültig.

## Modellzugriff

Mitglieder und Dienstkonten dürfen standardmäßig jedes Modell im Katalog aufrufen, Schlüssel jedes Modell, das ihr Inhaber darf. Im Sheet unter **Modelle** lassen sich stattdessen bestimmte Modelle wählen. Ein Schlüssel kann nie ein Modell aufrufen, das sein Mitglied nicht darf: Werden die Modelle eines Mitglieds eingeschränkt, schrumpfen die Listen seiner Schlüssel entsprechend.
