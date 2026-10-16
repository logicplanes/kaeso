---
title: Playground
description: Einen Prompt an bis zu vier Modelle nebeneinander senden.
order: 8
---

Der **Playground** sendet einen Prompt an ein oder mehrere Modelle des Projekts und zeigt ihre Antworten nebeneinander, bis zu vier gleichzeitig. Jede Spalte ist eine eigene Unterhaltung.

Playground-Anfragen sind echte Anfragen. Sie nutzen die Anbieter-Schlüssel des Projekts, den eigenen Modellzugriff und die eigenen Limits und Budgets sowie die des Projekts. Sie werden dem Projekt berechnet und erscheinen in den Anfrageprotokollen mit der Quelle **Playground**. Schlüssel-Limits gelten nicht, da kein Schlüssel verwendet wird.

## Nutzung

1. Ein Modell für die erste Spalte wählen; **Spalte hinzufügen** fügt weitere hinzu.
2. Einen Prompt eingeben und senden.
3. Jede Antwort zeigt die Zeit bis zum ersten Token, die Gesamtzeit, die Token und die Kosten.

**Leeren** leert die Unterhaltung jeder Spalte.

Ein Modell, für dessen Anbieter dieses Projekt keinen Schlüssel hat, lässt sich nicht wählen.

## Parameter

**Parameter** legt fest, was mit jeder Spalte gesendet wird:

| Parameter | Wirkung |
| --- | --- |
| Systemanweisungen | Anweisungen, die vor der Unterhaltung gesendet werden |
| Temperatur, Top P | Sampling, nur an Modelle gesendet, die eine Temperatur annehmen |
| Max. Ausgabe-Token | Die längste Antwort |
| Reasoning-Aufwand | Wie gründlich ein Reasoning-Modell nachdenkt, von ohne bis maximal |
| Reasoning-Zusammenfassung | Ob das Nachdenken eines Reasoning-Modells zusammengefasst wird: Auto, knapp, detailliert oder aus |

**Auf Standard zurücksetzen** setzt sie zurück.

Modelle und Parameter sind Teil der Adresse der Seite, ein Link darauf öffnet also denselben Vergleich. Ein noch nicht gesendeter Prompt bleibt im Browser erhalten; Unterhaltungen werden nicht gespeichert.

Den Playground zu nutzen erfordert das Recht **Playground nutzen**, das Projekt-Admins und Entwickler haben.
