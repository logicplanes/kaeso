---
title: Anfrageprotokolle
description: Jede Anfrage eines Projekts nachlesen, exportieren und festlegen, was gespeichert wird.
order: 7
---

Jede Modellanfrage wird protokolliert: Modell, Anbieter, Status, Token, Kosten, Zeiten, der Schlüssel und wer sie gesendet hat. Ist die Inhaltsprotokollierung eingeschaltet, werden auch Anfrage und Antwort gespeichert. Einträge erscheinen wenige Sekunden nach dem Ende einer Anfrage.

## Die Liste

Die Seite **Anfrageprotokolle** eines Projekts listet Anfragen, neueste zuerst, für die letzte Stunde, die letzten 24 Stunden, die letzten 7 oder 30 Tage oder einen gewählten Zeitraum.

- **Filter hinzufügen** grenzt die Liste ein nach Schlüssel, Mitglied, Modell, Anbieter, Status, Fehlerart, ob die Antwort gestreamt wurde, und nach der Quelle: API oder Playground. **Nur fehlgeschlagene** zeigt nur Fehler.
- **Live** fügt neue Anfragen hinzu, sobald sie eintreffen; ist die Liste nach unten gescrollt, zeigt ein Button, wie viele neu sind.
- Die Anzeigeoptionen wählen Spalten und Dichte.
- **Anfrage per ID öffnen** findet eine Anfrage über die `kaeso-request-id`, die die App erhalten hat.

## Eine Anfrage

Eine Anfrage wählen, um sie zu öffnen:

| Tab | Inhalt |
| --- | --- |
| Übersicht | Ergebnis, Route, Absender, Zeiten, Fortschritt des Streams und Nutzung |
| Anfrage | Was die App gesendet hat |
| Antwort | Was sie zurückbekommen hat |
| Rohdaten | Beides als JSON |

Prompts und Antworten erfordern das Recht **Prompts und Ausgaben ansehen**. Fehlt es, oder wurde der Inhalt nicht protokolliert oder ist seine Aufbewahrung abgelaufen, sagen die Tabs das. Embedding-Vektoren werden nie gespeichert. Eine Antwort, die in einem Format protokolliert wurde, das die Konsole noch nicht lesen kann, etwa von einem neueren Gateway, wird so gezeigt, wie sie gespeichert ist, mit einem Hinweis.

Eine fehlgeschlagene Anfrage zeigt ihre Fehlerart, etwa **Ratenlimit erreicht**, **Kontext zu lang**, **Zeitüberschreitung** oder **Client getrennt**, und die Meldung des Anbieters, sofern es eine gab. Eine gestreamte Antwort, die abbrach, wird als Fehler protokolliert, mit dem Stand, den sie erreicht hat.

## Export

**Exportieren** lädt die Anfragen, die den aktuellen Filtern entsprechen, als CSV, JSON Lines oder JSON Lines mit Inhalt herunter. Exportieren erfordert das Recht **Protokolle exportieren**, mit Inhalt zusätzlich **Prompts und Ausgaben ansehen**. In CSV bekommt ein Wert, der mit `=`, `+`, `-`, `@`, einem Tabulator oder einem Wagenrücklauf beginnt, ein vorangestelltes `'`, damit Tabellenkalkulationen ihn als Text zeigen und nicht als Formel ausführen.

## Aufbewahrung

Was wie lange gespeichert wird, ist pro Organisation unter **Organisationseinstellungen** › **Protokollierung** festgelegt:

| Einstellung | Standard | Wirkung |
| --- | --- | --- |
| Anfrageinhalte protokollieren | Ein | Ob Anfragen und Antworten mit ihren Protokolleinträgen gespeichert werden |
| Aufbewahrung der Metadaten | 90 Tage | Wie lange Einträge gespeichert werden |
| Aufbewahrung der Inhalte | 30 Tage | Wie lange Anfragen und Antworten gespeichert werden; nie länger als die Metadaten |

Die Installation legt die Standardwerte für neue Organisationen fest und die längste Aufbewahrung, die eine Organisation wählen darf. Eine kürzere Aufbewahrung löscht ältere Einträge; eine längere gilt für neue. Diese Einstellungen zu ändern erfordert das Recht **Protokollierung verwalten**.

Ein Projekt kann die Inhaltsprotokollierung für sich unter seinen **Einstellungen** abschalten, sie aber nicht einschalten, wenn die Organisation sie abgeschaltet hat.

## Speicher und Löschen

Der Tab **Speicher** unter **Protokollierung** zeigt, wie viele Einträge die Organisation hat und wie viel Platz sie belegen, pro Projekt. Auf dem Tab **Löschungen** löscht **Protokolle löschen** die Protokolle gewählter ganzer UTC-Tage, für alle Projekte oder eines, endgültig; Nutzung und Kosten bleiben erhalten. Zur Bestätigung `löschen` eingeben, und die Liste zeigt jede Löschung als **In Warteschlange** oder **Erledigt**. Löschen erfordert das Recht **Protokolle löschen** und eine kürzliche Anmeldung. Höchstens 10 Löschungen warten gleichzeitig; die nächste wird angenommen, sobald eine davon gelaufen ist.
