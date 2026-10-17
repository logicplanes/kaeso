---
title: Playground
description: Einen Prompt an bis zu vier Modelle nebeneinander senden.
order: 8
---

Der **Playground** sendet einen Prompt an bis zu vier Modelle des Projekts zugleich und zeigt ihre Antworten nebeneinander. Jede Spalte ist ein eigenes Gespräch. Ihn zu nutzen erfordert das Recht **Playground nutzen**, das Admins und Entwickler eines Projekts haben.

Anfragen im Playground sind echte Anfragen. Sie nutzen die Anbieter-Schlüssel des Projekts, den eigenen Modellzugriff sowie die eigenen Limits und die des Projekts; sie werden dem Projekt berechnet und erscheinen in den [Anfrageprotokollen](request-logs.md) mit der Quelle **Playground**. Schlüssel-Limits gelten nicht, weil kein Schlüssel verwendet wird.

## Modelle vergleichen

1. Ein Modell für die erste Spalte wählen und mit **Spalte hinzufügen** weitere. Ein Modell, dessen Anbieter keinen Schlüssel für dieses Projekt hat, lässt sich nicht wählen.
2. Einen Prompt an alle Modelle senden.
3. Jede Antwort zeigt die Zeit bis zum ersten Token, die Gesamtzeit, die Token und die Kosten.

**Leeren** leert das Gespräch jeder Spalte.

Modelle und Parameter sind Teil der Seitenadresse, sodass ein Link denselben Vergleich öffnet. Ein noch nicht gesendeter Prompt bleibt im Browser; Gespräche werden nicht gespeichert.

## Parameter

**Parameter** legt fest, was jede Spalte mitsendet:

| Parameter | Wirkung |
| --- | --- |
| Systemanweisungen | Anweisungen vor dem Gespräch |
| Temperatur, Top P | Sampling, nur an Modelle gesendet, die Temperatur unterstützen |
| Max. Ausgabe-Token | Die längste Antwort |
| Reasoning-Aufwand | Wie viel ein Reasoning-Modell nachdenkt, von **Ohne** bis **Maximal** |
| Reasoning-Zusammenfassung | Ob das Reasoning eines Modells zusammengefasst wird: **Auto**, **Knapp**, **Detailliert** oder **Aus** |

**Auf Standard zurücksetzen** setzt jeden Parameter außer den Systemanweisungen zurück.
