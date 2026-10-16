---
title: API-Schlüssel
description: Schlüssel für Apps und Skripte erstellen, einschränken und abschalten.
order: 5
---

Apps und Skripte rufen Kaeso mit einem **API-Schlüssel** auf. Ein Schlüssel gehört einem Mitglied oder einem Dienstkonto in einem Projekt, handelt dort mit dessen Rechten und lässt sich mit eigenem Modellzugriff und eigenen Limits weiter einschränken.

Ein Schlüssel besteht aus `sk-kso-` und 48 Buchstaben und Ziffern. Kaeso speichert nur einen Hash davon: Die Konsole zeigt ihn einmal, beim Erstellen, und danach nur seine ersten und letzten Zeichen.

## Schlüssel erstellen

Auf der Seite **API-Schlüssel** eines Projekts **Schlüssel erstellen** wählen.

1. **Wofür ist der Schlüssel?** Ein Name, etwa `checkout-web`, und wem er gehört: dem eigenen Konto oder, mit dem Recht **Dienstkonto-Schlüssel erstellen**, einem Dienstkonto des Projekts.
2. **Wann läuft er ab?** In 1, 7, 30 oder 90 Tagen, an einem Datum oder nie. Standard sind 90 Tage oder die längste erlaubte Wahl. Ein Schlüssel mit Datum endet zu Beginn dieses Tages, UTC. Organisation oder Projekt können eine längste Laufzeit festlegen, die längere Optionen und **Nie** ausblendet.
3. **Was darf er aufrufen?**
   - **Inferenz**: die Modell-API, für Apps.
   - **Plattform**: die Platform API, mit den Rechten des Inhabers in diesem Projekt, für Skripte.
   - **Inferenz und Plattform**: beides.
4. **Modelle und Limits**, optional: die Modelle, die er aufrufen darf, und eigene [Limits](limits-and-budgets.md).

Danach den Schlüssel kopieren. Der Dialog zeigt eine Beispielanfrage damit. Eigene Schlüssel zu erstellen erfordert das Recht **Eigene Schlüssel erstellen**; einem Schlüssel Plattformzugriff zu geben erfordert zusätzlich **Plattformzugriff vergeben** und eine kürzliche Anmeldung.

> [!TIP]
> Jeder App und jeder Umgebung einen eigenen Schlüssel geben. Nutzung, Protokolle und Limits gehören dann ihm allein, und wird er abgeschaltet, betrifft das nichts anderes.

## Das Sheet eines Schlüssels

Einen Schlüssel wählen, um sein Sheet zu öffnen: wem er gehört, sein Zugriff, wer ihn wann erstellt hat, sein Ablauf, seine letzte Nutzung und seine ID. Seine **Modelle** und **Limits** lassen sich dort jederzeit ändern, mit dem Recht **Limits verwalten**. **Inhaber der Schlüssel** über der Liste zeigt nur die Schlüssel eines Mitglieds oder Dienstkontos.

## Deaktivieren, aktivieren, archivieren

Das Menü im Sheet eines Schlüssels enthält diese Aktionen:

| Aktion | Wirkung |
| --- | --- |
| Deaktivieren | Der Schlüssel funktioniert sofort nicht mehr; er lässt sich jederzeit wieder aktivieren |
| Aktivieren | Ein deaktivierter Schlüssel funktioniert wieder |
| Archivieren | Der Schlüssel funktioniert endgültig nicht mehr; das lässt sich nicht rückgängig machen |

Eigene Schlüssel zu ändern erfordert das Recht **Eigene Schlüssel verwalten**, die Schlüssel aller **Alle Schlüssel verwalten**. Schlüssel enden auch, wenn sie ablaufen, wenn ihr Projekt archiviert wird und wenn ihr Mitglied das Projekt oder die Organisation verlässt. Die Seite gruppiert Schlüssel in aktiv, deaktiviert und abgelaufen; archivierte Schlüssel stehen unter **Alle**.

Ablaufende Schlüssel werden 7 Tage und 1 Tag vorher per E-Mail angekündigt: ihrem Inhaber oder, beim Schlüssel eines Dienstkontos, den Admins des Projekts. Jede Person kann diese E-Mails in ihrem Konto unter **Benachrichtigungen** abschalten.

## Die eigenen Schlüssel überall

**API-Schlüssel** im eigenen Konto listet die eigenen Schlüssel aus allen Projekten, um sie zu deaktivieren, zu aktivieren oder zu archivieren. Neue Schlüssel entstehen in einem Projekt.

## Plattform-Schlüssel

Die **Plattform-Schlüssel** einer Organisation, in ihren Einstellungen, sind für Skripte und Infrastruktur, die die ganze Organisation verwalten. Ein Plattform-Schlüssel beginnt mit `sk-kso-org-`, handelt mit den Rechten der Rolle Admin in der Organisation und jedem Projekt und ruft nie Modelle auf. **Plattform-Schlüssel erstellen** fragt nach einem Namen und einem Ablauf, mit denselben Optionen und der längsten Laufzeit der Organisation. Das Erstellen erfordert das Recht **Plattform-Schlüssel erstellen** und eine kürzliche Anmeldung. Plattform-Schlüssel werden wie API-Schlüssel deaktiviert, aktiviert und archiviert, mit dem Recht **Plattform-Schlüssel deaktivieren**.

Wie Skripte Schlüssel nutzen, steht unter [Platform API](../api/platform-api.md#authentifizierung).
