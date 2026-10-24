---
title: API-Schlüssel
description: Erstellen Sie Schlüssel für Apps und Skripte, begrenzen Sie, was sie dürfen, und schalten Sie sie ab.
order: 6
---

Apps und Skripte rufen Kaeso mit einem API-Schlüssel auf. Ein Schlüssel gehört einem Mitglied oder einem Dienstkonto in einem Projekt und handelt dort mit dessen Rechten. Eigener Modellzugriff und eigene Limits können ihn weiter einschränken.

## Schlüssel erstellen

1. Wählen Sie auf der Seite **API-Schlüssel** eines Projekts **Schlüssel erstellen**.
2. **Wofür ist der Schlüssel?** Geben Sie ihm einen Namen, etwa `checkout-web`. Anfrageprotokolle und Warnungen zeigen ihn. Wählen Sie unter **Für** sich selbst oder ein Dienstkonto des Projekts.
3. **Wann läuft er ab?** Wählen Sie 1, 7, 30 oder 90 Tage, **An einem Datum** oder **Nie**. Ein Schlüssel mit Datum endet zu Beginn dieses Tages, UTC.
4. **Was darf er aufrufen?** Wählen Sie **Inferenz** für Apps, die Modelle aufrufen, **Plattform** für Skripte, die das Projekt über die Platform API verwalten, oder **Inferenz und Plattform** für beides.
5. **Modelle und Limits**, optional: Wählen Sie die Modelle, die der Schlüssel aufrufen darf, und setzen Sie eigene [Limits](limits-and-budgets.md). Ein als [Nicht verfügbar](models-and-aliases.md) markiertes Modell ist ausgegraut, weil der Anbieter es dem Schlüssel des Projekts nicht bereitstellt. Ohne sie darf der Schlüssel aufrufen, was sein Inhaber darf, innerhalb der Limits seines Inhabers und des Projekts.

**Schlüssel erstellt** zeigt dann den Schlüssel und eine Beispielanfrage damit. Ein Schlüssel besteht aus `sk-kso-` und 48 Buchstaben und Ziffern, und danach zeigt die Konsole nur noch seine ersten und letzten Zeichen.

> [!IMPORTANT]
> Kopieren Sie den Schlüssel, bevor Sie den Dialog schließen. Kaeso speichert nur einen Hash davon und kann ihn nicht erneut anzeigen. Haben Sie ihn nicht kopiert, fragt die Konsole vor dem Schließen nach.

Standardmäßig läuft ein Schlüssel nach 90 Tagen ab, oder nach der längsten erlaubten Option. Setzt die Organisation oder das Projekt eine maximale Schlüssellaufzeit, sind längere Optionen und **Nie** ausgeblendet.

Einen Schlüssel zu erstellen erfordert diese Rechte:

| Zu erstellen | Recht |
| --- | --- |
| Eigene Schlüssel | **Eigene Schlüssel erstellen** |
| Schlüssel eines Dienstkontos | **Dienstkonto-Schlüssel erstellen** |
| Ein Schlüssel mit Plattformzugriff | Zusätzlich **Plattformzugriff vergeben**, eine Konsolensitzung und eine kürzliche Anmeldung; ein Schlüssel kann keinen erstellen |

Geben Sie jeder App und jeder Umgebung einen eigenen Schlüssel. Dann sind ihre Nutzung, Protokolle und Limits ihre eigenen, und das Abschalten eines Schlüssels betrifft nichts anderes.

## Schlüssel finden und ändern

**Aktiv** listet die Schlüssel, die funktionieren, **Archiviert** die archivierten und **Alle** jeden Schlüssel, auch deaktivierte und abgelaufene. **Inhaber der Schlüssel** über der Liste zeigt die Schlüssel eines Mitglieds oder Dienstkontos.

Wählen Sie einen Schlüssel, um sein Sheet zu öffnen. Es zeigt, wem der Schlüssel gehört, seinen Zugriff, wer ihn wann erstellt hat, seinen Ablauf, seine letzte Nutzung und seine ID. Mit dem Recht **Limits verwalten** können Sie dort jederzeit seine **Modelle** und **Limits** ändern.

## Schlüssel rotieren

Beim Rotieren ersetzt ein neuer Schlüssel mit demselben Namen, Inhaber, Zugriff, denselben Modellen und Limits den alten. Der alte Schlüssel funktioniert noch eine Schonfrist lang, die Sie wählen, damit Sie Ihre Apps ohne fehlgeschlagene Anfragen umstellen können. Rotieren Sie Schlüssel regelmäßig, oder sofort, wenn einer offengelegt sein könnte.

1. Wählen Sie im Menü im Sheet des Schlüssels **Rotieren**.
2. **Wann hört der alte Schlüssel auf zu funktionieren?** Wählen Sie **Sofort**, **Nach 1 Stunde**, **Nach 24 Stunden** oder **Nach 7 Tagen**.
3. **Wann läuft der neue Schlüssel ab?** Die Optionen sind dieselben wie beim Erstellen eines Schlüssels.

**Schlüssel rotiert** zeigt dann den neuen Schlüssel einmal an, wie **Schlüssel erstellt**. Bis zum Ende der Schonfrist funktionieren beide Schlüssel, dann läuft der alte ab. Würde der alte Schlüssel früher ablaufen, behält er seinen eigenen Ablauf, und Kaeso schickt für ihn keine Ablauf-E-Mails.

Das Sheet des alten Schlüssels zeigt **Ersetzt durch**, das des neuen **Rotiert aus**, jeweils mit einem Link zum anderen Schlüssel. Die Liste zeigt, wann ein ersetzter Schlüssel endet, und Anfrageprotokolle und Nutzung kennzeichnen ihn als ersetzt, damit Sie sehen, welche Apps ihn noch senden. Limits des neuen Schlüssels zählen von vorn.

Einen Schlüssel zu rotieren erfordert diese Rechte:

| Zu rotieren | Rechte |
| --- | --- |
| Ihr eigener Schlüssel | **Eigene Schlüssel erstellen** sowie **Eigene Schlüssel verwalten** oder **Alle Schlüssel verwalten** |
| Schlüssel eines Dienstkontos | **Dienstkonto-Schlüssel erstellen** und **Alle Schlüssel verwalten** |
| Ein Schlüssel mit Plattformzugriff | Zusätzlich **Plattformzugriff vergeben**, eine Konsolensitzung und eine kürzliche Anmeldung |

Den Schlüssel eines Mitglieds rotiert nur sein Inhaber, weil der neue Schlüssel mit dessen Rechten handelt. Ein archivierter oder bereits rotierter Schlüssel lässt sich nicht rotieren; rotieren Sie stattdessen seinen Nachfolger.

## Schlüssel abschalten

Neben **Rotieren** hat das Menü im Sheet eines Schlüssels drei Aktionen:

| Aktion | Wirkung |
| --- | --- |
| Deaktivieren | Der Schlüssel hört sofort auf zu funktionieren. Sie können ihn wieder aktivieren |
| Aktivieren | Ein deaktivierter Schlüssel funktioniert wieder. Hat eine Person ihn deaktiviert, aktiviert ihn nur eine Person, und unter **API-Schlüssel** im Konto lässt sich nur das eigene Deaktivieren rückgängig machen |
| Archivieren | Der Schlüssel hört endgültig auf zu funktionieren. Sein Verlauf bleibt erhalten |

Die eigenen Schlüssel zu ändern erfordert das Recht **Eigene Schlüssel verwalten**, die Schlüssel aller Personen zu ändern erfordert **Alle Schlüssel verwalten**.

Ein Schlüssel hört auch auf zu funktionieren, wenn er abläuft, wenn sein Projekt archiviert wird und wenn sein Mitglied das Projekt oder die Organisation verlässt. Kaeso schickt 7 Tage und 1 Tag vor dem Ablauf eine E-Mail. Sie geht an den Inhaber des Schlüssels, beim Schlüssel eines Dienstkontos an die Admins des Projekts. Jede Person kann diese E-Mails unter **Ablaufende Schlüssel** in den **Benachrichtigungen** ihres Kontos abschalten.

**API-Schlüssel** in Ihrem [Konto](account.md) listet Ihre eigenen Schlüssel aus allen Projekten, wo Sie sie deaktivieren, aktivieren oder archivieren können. Neue Schlüssel erstellen Sie in einem Projekt.

## Plattform-Schlüssel

Plattform-Schlüssel sind für Skripte und Infrastruktur, die die ganze Organisation verwalten, etwa Terraform. Ein Plattform-Schlüssel beginnt mit `sk-kso-org-`. Er handelt mit den Rechten der Rolle Admin in der Organisation und jedem Projekt und ruft nie Modelle auf.

1. Öffnen Sie in den **Organisationseinstellungen** die Seite **Plattform-Schlüssel** und wählen Sie **Plattform-Schlüssel erstellen**.
2. Geben Sie an, was den Schlüssel verwendet und wann er abläuft. Die Optionen und die maximale Schlüssellaufzeit der Organisation sind dieselben wie bei API-Schlüsseln.

Einen Plattform-Schlüssel zu erstellen erfordert das Recht **Plattform-Schlüssel erstellen**, eine Konsolensitzung und eine kürzliche Anmeldung. Plattform-Schlüssel deaktivieren, aktivieren und archivieren Sie wie API-Schlüssel, mit dem Recht **Plattform-Schlüssel deaktivieren**; hat eine Person einen Plattform-Schlüssel deaktiviert, aktiviert ihn nur eine Person.

**Rotieren** im Menü eines Plattform-Schlüssels funktioniert wie das [Rotieren eines API-Schlüssels](#schlüssel-rotieren) und behält den Namen des Schlüssels. Es erfordert beide Rechte, eine Konsolensitzung und eine kürzliche Anmeldung.

## Nächste Schritte

- [Modelle aufrufen](../get-started/calling-models.md): Senden Sie Ihre erste Anfrage mit dem neuen Schlüssel.
- [Limits und Budgets](limits-and-budgets.md): Begrenzen Sie, was ein Schlüssel anfragen und ausgeben darf.
- [Platform API](../api/platform-api.md): Verwalten Sie Projekte, Schlüssel und Modelle aus Skripten.
