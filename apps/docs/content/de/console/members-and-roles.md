---
title: Mitglieder und Rollen
description: Laden Sie Personen ein, geben Sie ihnen Rollen und Rechte, und erstellen Sie Dienstkonten für Software.
order: 2
---

Jede Person in einer Organisation hat eine Organisationsrolle und in jedem Projekt, dem sie angehört, eine Projektrolle. Eine Rolle ist eine Menge von Rechten, und Sie können einem Mitglied zusätzlich einzelne Rechte geben oder nehmen. [Rechte](../concepts/rights.md) listet jedes Recht und die Rollen, die es enthalten.

Was Sie in der Konsole sehen, hängt von Ihren Rechten ab. Eine Seite, für die Ihnen das Recht fehlt, erscheint nicht in der Seitenleiste. Eine Aktion, die Sie nicht ausführen dürfen, ist deaktiviert, und ihr Tooltip nennt das nötige Recht und wer es Ihnen geben kann.

## Personen einladen

Öffnen Sie **Organisationseinstellungen** › **Mitglieder** und wählen Sie **Mitglied einladen**. Beantworten Sie dann drei Schritte:

1. **Wer wird eingeladen?** Geben Sie eine Adresse ein und drücken Sie Enter, oder fügen Sie mehrere ein.
2. **Was dürfen sie in der Organisation?** Wählen Sie ihre Organisationsrolle.
3. **Welche Projekte?** Fügen Sie die Projekte hinzu, denen sie beitreten, jeweils mit einer Rolle. Dem Standardprojekt treten sie immer bei.

**Einladung senden** schickt jeder Person einen Link, der 21 Tage gilt. **Einladungen** listet die offenen Einladungen, die Sie mit **Erneut senden** oder **Widerrufen** bearbeiten. Angenommene, widerrufene und abgelaufene Einladungen stehen unter **Alle**.

Eine Organisation mit [Single Sign-on](organizations-and-projects.md#single-sign-on) kann Personen ihrer E-Mail-Domains auch ohne Einladung beitreten lassen. Das gilt nur für Personen, die neu bei Kaeso sind. Wer schon ein Konto hat, braucht eine Einladung.

## Rolle wählen

Organisationen haben sechs integrierte Rollen: **Inhaber**, **Admin**, **Abrechnung**, **Entwickler**, **Prüfer** und **Mitglied**. Projekte haben drei: **Admin**, **Entwickler** und **Betrachter**. Die Konsole beschreibt jede Rolle dort, wo Sie eine wählen, und integrierte Rollen lassen sich nicht ändern.

- Inhaber haben jedes Recht, auch die Rechte, die spätere Versionen hinzufügen. Nur Inhaber ernennen, ändern oder entfernen Inhaber, und eine Organisation behält immer mindestens einen.
- Inhaber und Admins haben jedes Recht in jedem Projekt.
- Im Standardprojekt beginnen Inhaber und Admins als Admin, Entwickler als Entwickler und alle anderen als Betrachter.

Für jede Ihrer Änderungen gelten zwei Regeln. Sie können nur Rechte vergeben, die Sie selbst haben. Und Sie können nur Mitglieder ändern, die kein Recht haben, das Ihnen fehlt.

## Rolle oder Rechte eines Mitglieds ändern

Wählen Sie unter **Mitglieder** ein Mitglied, um sein Sheet zu öffnen:

- **Rolle** zeigt seine Rolle. **Anpassen** gibt oder nimmt einzelne Rechte. Ein Recht, das ein anderes seiner Rechte voraussetzt, kommt und geht mit diesem.
- **Zugriff** listet jedes Recht des Mitglieds und die Rolle oder das Recht, aus dem es stammt.
- Unter seinen Projekten steht seine Rolle in jedem Projekt.
- **Inhaberschaft übertragen**, für Inhaber, macht das Mitglied zum Inhaber und Sie zum Admin.
- **Aus der Organisation entfernen** nimmt es aus jedem Projekt und archiviert seine Schlüssel in der Organisation. In Ihrem eigenen Sheet heißt das **Organisation verlassen**.

## Eigene Rolle erstellen

Öffnen Sie **Organisationseinstellungen** › **Mitglieder** › **Rollen** und wählen Sie **Rolle erstellen**. Eine Rolle gilt für die Organisation oder für Projekte und beginnt bei einer integrierten Rolle oder ohne Rechte. Eine Organisationsrolle kann zusätzlich Rechte in jedem Projekt geben. Um bei einer integrierten Rolle zu beginnen, öffnen Sie diese und wählen Sie **Duplizieren**.

- Eine Organisation hat höchstens 50 eigene Rollen.
- Eine Änderung an einer Rolle gilt sofort für alle, die sie haben.
- Die Konsole lehnt eine Änderung ab, die einem Träger der Rolle ein Recht ohne ein Recht ließe, das dieses voraussetzt. Die Meldung nennt beide Rechte und wie viele Träger betroffen sind.
- Eine Rolle, die jemand hat, können Sie nicht löschen. Geben Sie den Personen zuerst eine andere Rolle.

## Personen zu einem Projekt hinzufügen

Wählen Sie auf der Seite **Mitglieder** eines Projekts **Mitglied hinzufügen** und wählen Sie jemanden aus der Organisation mit einer Projektrolle. Laden Sie Personen außerhalb der Organisation zuerst in die Organisation ein.

Das Sheet eines Projektmitglieds enthält auch seinen [Modellzugriff](#modelle-einschränken-die-jemand-aufrufen-darf) und seine [Limits](limits-and-budgets.md). **Aus Projekt entfernen** archiviert seine Schlüssel im Projekt, und in der Organisation bleibt es.

## Dienstkonto erstellen

Ein Dienstkonto ist ein Projektmitglied für Software, etwa ein Deployment, einen Job oder eine Pipeline. Seine Schlüssel funktionieren weiter, wenn die Personen gehen, die sie erstellt haben.

1. Wählen Sie auf der Seite **Dienstkonten** eines Projekts **Dienstkonto erstellen**.
2. Geben Sie an, was es verwendet, etwa `checkout-production`.
3. Wählen Sie seine Projektrolle. Seine Schlüssel handeln mit den Rechten dieser Rolle.

Sein Sheet enthält seine Schlüssel mit **Schlüssel erstellen**, seinen Modellzugriff und seine Limits. **Dienstkonto archivieren** archiviert es samt Schlüsseln endgültig.

Ein Dienstkonto zu erstellen erfordert das Recht **Dienstkonten erstellen**, seine Schlüssel zu erstellen **Dienstkonto-Schlüssel erstellen**.

## Modelle einschränken, die jemand aufrufen darf

Mitglieder und Dienstkonten dürfen jedes Modell im Katalog des Projekts aufrufen, ein Schlüssel jedes Modell, das sein Inhaber aufrufen darf. Um das einzuschränken, öffnen Sie **Modelle** im Sheet eines Mitglieds, Dienstkontos oder Schlüssels und wählen Sie bestimmte Modelle.

Ein Schlüssel ruft nie ein Modell auf, das sein Inhaber nicht darf. Wenn Sie die Modelle eines Mitglieds einschränken, schränken sich die Listen seiner Schlüssel mit ein. Den Modellzugriff zu ändern erfordert das Recht **Limits verwalten**.

## Nächste Schritte

- [API-Schlüssel](api-keys.md): Erstellen Sie Schlüssel für Mitglieder und Dienstkonten.
- [Limits und Budgets](limits-and-budgets.md): Begrenzen Sie, was ein Mitglied, ein Dienstkonto oder ein Schlüssel ausgeben darf.
- [Rechte](../concepts/rights.md): Schlagen Sie jedes Recht nach und die Rollen, die es enthalten.
