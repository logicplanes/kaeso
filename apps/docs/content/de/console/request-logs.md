---
title: Anfrageprotokolle
description: Finden Sie jede Anfrage eines Projekts, sehen Sie Gesendetes und Erhaltenes, exportieren Sie Protokolle und wählen Sie, was bleibt.
order: 8
---

Kaeso protokolliert jede Modellanfrage: das Modell, den Anbieter, den Status, Token, Kosten, Zeiten, den Schlüssel und wer sie gesendet hat. Mit eingeschalteter Inhaltsprotokollierung speichert es auch, was die App gesendet und zurückbekommen hat. Eine Anfrage erscheint wenige Sekunden nach ihrem Ende.

## Anfrage finden

Öffnen Sie die Seite **Anfrageprotokolle** eines Projekts. Sie listet Anfragen, die neuesten zuerst, aus der letzten Stunde, den letzten 24 Stunden, 7 Tagen oder 30 Tagen oder einem selbst gewählten Zeitraum.

- **Filter hinzufügen** grenzt die Liste nach Schlüssel, Mitglied, Modell, Anbieter, Status, Fehlerart, Streaming und Quelle ein: die API oder der Playground. **Nur fehlgeschlagene** behält die Fehler.
- **Live** fügt neue Anfragen hinzu, sobald sie eintreffen. Wenn Sie nach unten gescrollt haben, nennt ein Button die Zahl der neuen.
- Die Anzeigeoptionen wählen die Spalten und die Dichte.
- **Anfrage per ID öffnen** findet die Anfrage, deren Header `kaeso-request-id` Ihre App erhalten hat.

Die Protokolle zu lesen erfordert das Recht **Anfrageprotokolle ansehen**.

## Anfrage lesen

Wählen Sie eine Anfrage, um sie zu öffnen:

| Tab | Inhalt |
| --- | --- |
| Übersicht | Das Ergebnis, die Route, wer sie gesendet hat, die Zeiten, der Stream und die Nutzung |
| Anfrage | Was die App gesendet hat |
| Antwort | Was die App zurückbekommen hat |
| Rohdaten | Der Protokolleintrag, wie die Platform API ihn liefert |

Die Zeiten trennen die Arbeit von Kaeso von der des Anbieters. **Kaeso-Overhead** ist die Zeit, bevor Kaeso den Anbieter aufrief: Body lesen, Schlüssel prüfen, übersetzen und Limits prüfen. Bei einem Stream ist **Stream-Verarbeitung** die Zeit, in der Kaeso die Events des Anbieters für die App übersetzt hat, ohne die Wartezeiten auf beide Seiten. Der CSV-Export enthält beides, als `overhead_ms` und `stream_processing_ms`.

Eine fehlgeschlagene Anfrage zeigt ihre Fehlerart, etwa **Ratenlimit erreicht**, **Kontext zu lang**, **Zeitüberschreitung**, **Client getrennt** oder **Client liest nicht mehr**. Hat der Anbieter eine Meldung gesendet, zeigt sie auch diese. Ein abgebrochener Stream wird als Fehler protokolliert, mit dem Stand, den er erreicht hat.

Die Tabs **Anfrage** und **Antwort** erfordern das Recht **Prompts und Ausgaben ansehen**. Sie sagen Ihnen, wenn dieses Recht fehlt, wenn kein Inhalt protokolliert wurde oder wenn seine Aufbewahrung abgelaufen ist. Embedding-Vektoren speichert Kaeso nie. Eine Antwort, die in einem Format protokolliert wurde, das diese Konsole noch nicht lesen kann (etwa von einem neueren Gateway), erscheint so, wie sie gespeichert wurde.

## Protokolle exportieren

**Exportieren** lädt die Anfragen herunter, die zu den aktuellen Filtern passen:

| Format | Enthält |
| --- | --- |
| CSV | Metadaten, für Tabellenkalkulationen |
| JSON Lines | Metadaten, eine Anfrage pro Zeile |
| JSON Lines mit Inhalt | Metadaten und die gespeicherten Anfragen und Antworten |

Exportieren erfordert das Recht **Protokolle exportieren**, mit Inhalt zusätzlich **Prompts und Ausgaben ansehen**.

In CSV setzt Kaeso jeden Textwert in doppelte Anführungszeichen. Ein `'` setzt es vor jeden Wert, der mit `=`, `+`, `-`, `@`, einem Tabulator oder einem Wagenrücklauf beginnt. Dasselbe gilt für einen Wert mit `;`, `|` oder einem Tabulator und nach einem solchen Trennzeichen, wenn eines dieser Zeichen folgt. Eine Tabellenkalkulation zeigt den Wert dann als Text, statt ihn als Formel auszuführen, an welchem Trennzeichen sie auch teilt.

## Aufbewahrung

Jede Organisation legt unter **Organisationseinstellungen** › **Protokollierung** fest, was sie aufbewahrt und wie lange:

| Einstellung | Standard | Wirkung |
| --- | --- | --- |
| Anfrageinhalte protokollieren | An | Ob Anfragen und Antworten mit ihren Protokolleinträgen gespeichert werden |
| Aufbewahrung der Metadaten | 90 Tage | Wie lange Protokolleinträge bleiben |
| Aufbewahrung der Inhalte | 30 Tage | Wie lange Anfragen und Antworten bleiben. Nie länger als die Metadaten |

Eine kürzere Aufbewahrung löscht ältere Einträge, eine längere gilt für neue. Die Installation legt die Standards für neue Organisationen fest und die längste Aufbewahrung, die eine Organisation wählen darf ([Instanzeinstellungen](instance-administration.md#installationseinstellungen-ändern)). Diese Einstellungen zu ändern erfordert das Recht **Protokollierung verwalten**.

Ein Projekt kann die Inhaltsprotokollierung für sich in seinen **Einstellungen** ausschalten, mit dem Recht **Inhaltsprotokollierung schalten**. Einschalten kann es sie nicht, solange die Organisation sie ausgeschaltet hat.

## Protokolle löschen

Der Tab **Speicher** unter **Protokollierung** zeigt, wie viele Einträge die Organisation aufbewahrt und wie viel Platz sie je Projekt belegen. Er und der Tab **Löschungen** erfordern das Recht **Protokollspeicher ansehen**.

1. Wählen Sie auf dem Tab **Löschungen** **Protokolle löschen**.
2. Wählen Sie ganze UTC-Tage und alle Projekte oder eines.
3. Geben Sie zur Bestätigung `delete` ein.

Kaeso löscht die Protokolle dieser Tage endgültig und behält Nutzung und Kosten. Die Liste zeigt jede Löschung als **In Warteschlange** oder **Erledigt**, und höchstens 10 Löschungen warten zugleich. Sie listet auch die Löschungen, die eine kürzere Aufbewahrung startet, als **Kürzere Aufbewahrung der Metadaten** oder **Kürzere Aufbewahrung der Inhalte**. Eine Löschung, die fehlgeschlagen ist und erneut laufen wird, zeigt „Neuer Versuch:“ und den Fehler. Löschen erfordert das Recht **Protokolle löschen** und eine kürzliche Anmeldung.

## Nächste Schritte

- [Organisationen und Projekte](organizations-and-projects.md): Schalten Sie die Inhaltsprotokollierung für ein einzelnes Projekt aus.
- [Fehler](../api/errors.md): Schlagen Sie nach, was eine fehlgeschlagene Anfrage der App zurückgibt.
- [Anforderungen](../self-hosting/requirements.md): Planen Sie den Speicherplatz, den Anfrageprotokolle belegen.
