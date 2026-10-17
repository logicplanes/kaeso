---
title: API-Schlüssel
description: Schlüssel für Apps und Skripte erstellen, einschränken und abschalten.
order: 5
---

Apps und Skripte rufen Kaeso mit einem **API-Schlüssel** auf. Ein Schlüssel gehört einem Mitglied oder einem Dienstkonto in einem Projekt und handelt dort mit dessen Rechten; eigener Modellzugriff und eigene Limits können ihn weiter einschränken.

Ein Schlüssel besteht aus `sk-kso-` und 48 Buchstaben und Ziffern. Kaeso speichert nur einen Hash davon: Die Konsole zeigt den ganzen Schlüssel einmal, beim Erstellen, und danach nur seine ersten und letzten Zeichen.

## Schlüssel erstellen

Auf der Seite **API-Schlüssel** eines Projekts **Schlüssel erstellen** wählen und vier Schritte ausfüllen:

1. **Wofür ist der Schlüssel?** Ein Name, etwa `checkout-web`, der in Anfrageprotokollen und Warnungen erscheint, und **Für**: die eigene Person oder ein Dienstkonto des Projekts.
2. **Wann läuft er ab?** In 1, 7, 30 oder 90 Tagen, **An einem Datum** oder **Nie**. Standard sind 90 Tage oder die längste erlaubte Wahl. Ein Schlüssel mit Datum endet zu Beginn dieses Tages, UTC. Eine maximale Schlüssellaufzeit der Organisation oder des Projekts blendet längere Optionen und **Nie** aus.
3. **Was darf er aufrufen?**
   - **Inferenz**: die Modell-API, für Apps.
   - **Plattform**: die Platform API, mit den Rechten des Inhabers in diesem Projekt, für Skripte.
   - **Inferenz und Plattform**: beides.
4. **Modelle und Limits**, optional: die Modelle, die er aufrufen darf, und eigene [Limits](limits-and-budgets.md). Ohne sie darf er, was sein Inhaber darf, innerhalb der Limits des Inhabers und des Projekts.

**Schlüssel erstellt** zeigt danach den Schlüssel mit einer Beispielanfrage. Ihn kopieren, bevor der Dialog geschlossen wird: Beim Schließen vergisst die Konsole ihn und kann ihn nicht erneut zeigen; wurde er noch nicht kopiert, fragt sie vorher nach.

| Zum Erstellen | Recht |
| --- | --- |
| Eigener Schlüssel | **Eigene Schlüssel erstellen** |
| Schlüssel eines Dienstkontos | **Dienstkonto-Schlüssel erstellen** |
| Schlüssel mit Plattformzugriff | Zusätzlich **Plattformzugriff vergeben** und eine kürzliche Anmeldung |

> [!TIP]
> Jeder App und jeder Umgebung einen eigenen Schlüssel geben. Nutzung, Protokolle und Limits gehören dann ihm allein, und wird er abgeschaltet, betrifft das nichts anderes.

## Die Schlüsselliste und das Sheet eines Schlüssels

**Aktiv** listet die Schlüssel, die funktionieren; **Alle** zeigt zusätzlich deaktivierte, abgelaufene und archivierte. **Inhaber der Schlüssel** über der Liste zeigt nur die Schlüssel eines Mitglieds oder Dienstkontos.

Einen Schlüssel wählen, um sein Sheet zu öffnen: wem er gehört, sein Zugriff, wer ihn wann erstellt hat, sein Ablauf, seine letzte Nutzung und seine ID. Seine **Modelle** und **Limits** lassen sich dort jederzeit ändern, mit dem Recht **Limits verwalten**.

## Deaktivieren, aktivieren, archivieren

| Aktion | Wirkung |
| --- | --- |
| Deaktivieren | Der Schlüssel funktioniert sofort nicht mehr; er lässt sich wieder aktivieren |
| Aktivieren | Ein deaktivierter Schlüssel funktioniert wieder |
| Archivieren | Der Schlüssel funktioniert endgültig nicht mehr; das lässt sich nicht rückgängig machen, sein Verlauf bleibt erhalten |

Die Aktionen stehen im Menü des Sheets. Eigene Schlüssel zu ändern erfordert das Recht **Eigene Schlüssel verwalten**, die Schlüssel aller **Alle Schlüssel verwalten**.

Ein Schlüssel endet außerdem, wenn er abläuft, wenn sein Projekt archiviert wird und wenn sein Mitglied das Projekt oder die Organisation verlässt. 7 Tage und 1 Tag vor dem Ablauf schickt Kaeso eine E-Mail: an den Inhaber oder, beim Schlüssel eines Dienstkontos, an die Admins des Projekts. Jede Person kann diese E-Mails in ihrem Konto unter **Benachrichtigungen** abschalten.

**API-Schlüssel** im eigenen Konto listet die eigenen Schlüssel aus allen Projekten, um sie zu deaktivieren, zu aktivieren oder zu archivieren; neue Schlüssel entstehen in einem Projekt.

## Plattform-Schlüssel

**Plattform-Schlüssel** in den Einstellungen der Organisation sind für Skripte und Infrastruktur, die die ganze Organisation verwalten, etwa Terraform. Ein Plattform-Schlüssel beginnt mit `sk-kso-org-`, handelt mit den Rechten der Rolle Admin in der Organisation und jedem Projekt und ruft nie Modelle auf.

**Plattform-Schlüssel erstellen** fragt, wofür er verwendet wird und wann er abläuft, mit denselben Optionen und der maximalen Schlüssellaufzeit der Organisation. Das erfordert das Recht **Plattform-Schlüssel erstellen** und eine kürzliche Anmeldung. Plattform-Schlüssel werden wie API-Schlüssel deaktiviert, aktiviert und archiviert, mit dem Recht **Plattform-Schlüssel deaktivieren**.

Wie Skripte sich mit Schlüsseln anmelden, erklärt [Platform API](../api/platform-api.md).
