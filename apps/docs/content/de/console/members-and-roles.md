---
title: Mitglieder und Rollen
description: Personen einladen, ihnen Rollen und Rechte geben und Dienstkonten erstellen.
order: 2
---

Jede Person in einer Organisation hat eine **Organisationsrolle** und in jedem Projekt, dem sie angehört, eine **Projektrolle**. Eine Rolle ist eine Menge von Rechten; einem Mitglied lassen sich zusätzlich einzelne Rechte geben oder nehmen. [Rechte](../concepts/rights.md) listet jedes Recht und was jede Rolle enthält.

## Rollen

Organisationen haben sechs eingebaute Rollen, **Inhaber**, **Admin**, **Abrechnung**, **Entwickler**, **Prüfer** und **Mitglied**, Projekte drei: **Admin**, **Entwickler** und **Betrachter**. Die Konsole beschreibt jede Rolle dort, wo eine gewählt wird. Eingebaute Rollen lassen sich nicht ändern.

- Inhaber haben jedes Recht, auch die künftiger Versionen. Nur Inhaber ernennen, ändern oder entfernen Inhaber, und eine Organisation behält immer mindestens einen.
- Inhaber und Admins haben jedes Recht in jedem Projekt.
- Im Standardprojekt beginnen Inhaber und Admins als Admin, Entwickler als Entwickler und alle anderen als Betrachter.

Für jede Änderung gelten zwei Regeln: Vergeben lassen sich nur Rechte, die man selbst hat, und ändern lassen sich nur Mitglieder, die kein Recht haben, das einem selbst fehlt.

### Eigene Rollen

Unter **Organisationseinstellungen** › **Mitglieder** › **Rollen** legt **Rolle erstellen** eine Rolle für die Organisation oder für Projekte an, ausgehend von einer eingebauten Rolle oder von keinen Rechten. Eine Organisationsrolle kann zusätzlich Rechte in jedem Projekt geben. Um von einer eingebauten Rolle auszugehen, diese öffnen und **Duplizieren** wählen.

- Eine Organisation hat höchstens 50 eigene Rollen.
- Eine Änderung gilt sofort für alle, die die Rolle haben. Sie wird nicht gespeichert, wenn sie jemandem ein Recht ließe, ohne ein Recht, das dieses voraussetzt; die Meldung nennt beide Rechte und wie viele Personen betroffen sind.
- Eine Rolle, die jemand hat, lässt sich nicht löschen; vorher eine andere Rolle vergeben.

## Personen einladen

Unter **Organisationseinstellungen** › **Mitglieder** öffnet **Mitglied einladen** drei Schritte:

1. **Wer wird eingeladen?** Eine Adresse eingeben und Enter drücken, oder mehrere einfügen.
2. **Was dürfen sie in der Organisation?** Ihre Organisationsrolle.
3. **Welche Projekte?** Projekte, denen sie beitreten, jeweils mit einer Rolle. Dem Standardprojekt treten sie immer bei.

**Einladung senden** schickt jeder Person einen Link, der 21 Tage gilt. **Einladungen** listet offene Einladungen, mit **Erneut senden** und **Widerrufen**; angenommene, widerrufene und abgelaufene stehen unter **Alle**.

Eine Organisation mit [Single Sign-on](organizations-and-projects.md#single-sign-on) kann Personen ihrer E-Mail-Domains auch ohne Einladung beitreten lassen. Das gilt nur für Personen ohne Kaeso-Konto; wer schon ein Konto hat, braucht eine Einladung.

## Das Sheet eines Mitglieds

Ein Mitglied wählen, um sein Sheet zu öffnen:

- **Rolle**, mit **Anpassen**, um einzelne Rechte zu geben oder zu nehmen. Ein Recht, das ein anderes seiner Rechte voraussetzt, kommt und geht zusammen mit diesem.
- **Zugriff**: jedes Recht des Mitglieds und die Rolle oder das Recht, aus dem es stammt.
- Seine Projekte, mit seiner Rolle in jedem.
- **Inhaberschaft übertragen**, für Inhaber: Das Mitglied wird Inhaber, man selbst Admin.
- **Aus der Organisation entfernen**: Das Mitglied verlässt jedes Projekt, und seine Schlüssel in der Organisation werden archiviert. Im eigenen Sheet heißt das **Organisation verlassen**.

## Projektmitglieder

Auf der Seite **Mitglieder** eines Projekts fügt **Mitglied hinzufügen** jemanden aus der Organisation mit einer Projektrolle hinzu; andere Personen zuerst in die Organisation einladen. Das Sheet eines Projektmitglieds enthält auch seinen [Modellzugriff](#modellzugriff) und seine [Limits](limits-and-budgets.md). **Aus Projekt entfernen** archiviert seine Schlüssel im Projekt; in der Organisation bleibt es.

## Dienstkonten

Ein **Dienstkonto** ist ein Projektmitglied für Software, etwa ein Deployment, einen Job oder eine Pipeline. Seine Schlüssel funktionieren weiter, egal wer die Organisation verlässt.

Auf der Seite **Dienstkonten** eines Projekts fragt **Dienstkonto erstellen**, wofür es verwendet wird, etwa `checkout-production`, und nach seiner Projektrolle, mit der seine Schlüssel handeln. Sein Sheet enthält seine Schlüssel mit **Schlüssel erstellen**, seinen Modellzugriff und seine Limits. **Dienstkonto archivieren** archiviert es samt Schlüsseln dauerhaft.

Ein Dienstkonto zu erstellen erfordert das Recht **Dienstkonten erstellen**, seine Schlüssel zu erstellen **Dienstkonto-Schlüssel erstellen**.

## Modellzugriff

Mitglieder und Dienstkonten dürfen jedes Modell im Katalog des Projekts aufrufen, Schlüssel jedes Modell, das ihr Inhaber darf. Unter **Modelle** im Sheet eines Mitglieds, Dienstkontos oder Schlüssels lassen sich stattdessen bestimmte Modelle wählen. Ein Schlüssel ruft nie ein Modell auf, das sein Inhaber nicht darf: Wer die Modelle eines Mitglieds einschränkt, schränkt die Modelllisten seiner Schlüssel mit ein. Den Modellzugriff zu ändern erfordert das Recht **Limits verwalten**.
