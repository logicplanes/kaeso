---
title: Anfrageprotokolle
description: Jede Anfrage eines Projekts nachlesen, exportieren und festlegen, was gespeichert wird.
order: 7
---

Kaeso protokolliert jede Modellanfrage: Modell, Anbieter, Status, Token, Kosten, Zeiten, den Schlüssel und wer sie gesendet hat. Bei eingeschalteter Inhaltsprotokollierung speichert es auch, was die App gesendet und zurückbekommen hat. Ein Eintrag erscheint wenige Sekunden nach dem Ende seiner Anfrage. Die Protokolle zu lesen erfordert das Recht **Anfrageprotokolle ansehen**.

## Die Liste

Die Seite **Anfrageprotokolle** eines Projekts listet Anfragen, die neuesten zuerst, aus der letzten Stunde, den letzten 24 Stunden, 7 oder 30 Tagen oder einem eigenen Zeitraum.

- **Filter hinzufügen** grenzt die Liste nach Schlüssel, Mitglied, Modell, Anbieter, Status, Fehlerart, Streaming und Quelle ein (die API oder der Playground); **Nur fehlgeschlagene** zeigt nur Fehler.
- **Live** fügt neue Anfragen hinzu, sobald sie eintreffen. Wer nach unten gescrollt hat, sieht eine Schaltfläche mit der Zahl neuer Anfragen.
- Die Anzeigeoptionen wählen Spalten und Dichte.
- **Anfrage per ID öffnen** findet die Anfrage zur `kaeso-request-id`, die die App erhalten hat.

## Eine Anfrage

Eine Anfrage wählen, um sie zu öffnen:

| Tab | Inhalt |
| --- | --- |
| Übersicht | Ergebnis, Route, Absender, Zeiten, Stream und Nutzung |
| Anfrage | Was die App gesendet hat |
| Antwort | Was sie zurückbekommen hat |
| Rohdaten | Der Protokolleintrag, wie die Platform API ihn liefert |

Eine fehlgeschlagene Anfrage zeigt ihre Fehlerart, etwa **Ratenlimit erreicht**, **Kontext zu lang**, **Zeitüberschreitung** oder **Client getrennt**, und die Meldung des Anbieters, falls es eine gab. Ein abgebrochener Stream wird als Fehler protokolliert, mit dem Stand, bis zu dem er kam.

Die Tabs **Anfrage** und **Antwort** erfordern das Recht **Prompts und Ausgaben ansehen**. Sie sagen es, wenn es fehlt, wenn kein Inhalt protokolliert wurde oder wenn seine Aufbewahrung abgelaufen ist. Embedding-Vektoren werden nie gespeichert. Eine Antwort in einem Format, das diese Konsole noch nicht lesen kann, etwa von einem neueren Gateway, wird so gezeigt, wie sie gespeichert ist.

## Export

**Exportieren** lädt die Anfragen herunter, die den aktuellen Filtern entsprechen:

| Format | Inhalt |
| --- | --- |
| CSV | Metadaten, für Tabellenkalkulationen |
| JSON Lines | Metadaten, eine Anfrage pro Zeile |
| JSON Lines mit Inhalt | Zusätzlich die gespeicherten Anfragen und Antworten |

Exportieren erfordert das Recht **Protokolle exportieren**, mit Inhalt zusätzlich **Prompts und Ausgaben ansehen**. In CSV erhält ein Wert, der mit `=`, `+`, `-`, `@`, einem Tabulator oder einem Wagenrücklauf beginnt, ein vorangestelltes `'`, damit eine Tabellenkalkulation ihn als Text zeigt, statt ihn als Formel auszuführen.

## Aufbewahrung

Jede Organisation legt unter **Organisationseinstellungen** › **Protokollierung** fest, was wie lange gespeichert wird:

| Einstellung | Standard | Wirkung |
| --- | --- | --- |
| Anfrageinhalte protokollieren | An | Ob Anfragen und Antworten mit ihren Protokolleinträgen gespeichert werden |
| Aufbewahrung der Metadaten | 90 Tage | Wie lange Einträge gespeichert werden |
| Aufbewahrung der Inhalte | 30 Tage | Wie lange Anfragen und Antworten gespeichert werden; nie länger als die Metadaten |

Die Installation legt die Standardwerte für neue Organisationen fest und die längste Aufbewahrung, die eine Organisation wählen darf ([Instanzeinstellungen](instance-administration.md#einstellungen)). Eine kürzere Aufbewahrung löscht ältere Einträge; eine längere gilt für neue. Diese Einstellungen zu ändern erfordert das Recht **Protokollierung verwalten**.

Ein Projekt kann die Inhaltsprotokollierung für sich unter **Einstellungen** ausschalten, aber nicht einschalten, solange die Organisation sie ausgeschaltet hat.

## Speicher und Löschen

Der Tab **Speicher** unter **Protokollierung** zeigt, wie viele Einträge die Organisation hat und wie viel Platz sie belegen, je Projekt.

Auf dem Tab **Löschungen** löscht **Protokolle löschen** die Protokolle ganzer UTC-Tage, aus allen Projekten oder einem, endgültig; Nutzung und Kosten bleiben. Bestätigt wird durch Eingabe von `delete`, und die Liste zeigt jede Löschung als **In Warteschlange** oder **Erledigt**. Höchstens 10 Löschungen warten gleichzeitig. Löschen erfordert das Recht **Protokolle löschen** und eine kürzliche Anmeldung.
