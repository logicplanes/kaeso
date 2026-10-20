---
title: Playground
description: Senden Sie einen Prompt an bis zu vier Modelle und vergleichen Sie ihre Antworten nebeneinander.
order: 8
---

Der **Playground** sendet einen Prompt an bis zu vier Modelle des Projekts zugleich und zeigt ihre Antworten nebeneinander. Jede Spalte ist ein eigenes Gespräch. Ihn zu nutzen erfordert das Recht **Playground nutzen**, das Admins und Entwickler eines Projekts haben.

## Modelle vergleichen

1. Wählen Sie ein Modell für die erste Spalte und mit **Spalte hinzufügen** jede weitere. Suchen Sie nach Name, ID oder Anbieter; die Modelle sind nach Anbieter gruppiert, und die zuletzt gewählten stehen oben. Ein Modell, dessen Anbieter keinen Schlüssel für dieses Projekt hat, können Sie nicht wählen.
2. Senden Sie einen Prompt an alle Modelle.
3. Lesen Sie die Antworten. Jede zeigt die Zeit bis zum ersten Token, die Gesamtzeit, die Token und die Kosten.

**Leeren** leert das Gespräch jeder Spalte.

Die Adresse der Seite enthält die Modelle und Parameter, sodass ein Link denselben Vergleich öffnet. Ein Prompt, den Sie noch nicht gesendet haben, bleibt in Ihrem Browser. Gespräche werden nicht gespeichert.

## Parameter festlegen

**Parameter** legt fest, was jede Spalte mitsendet:

| Parameter | Wirkung |
| --- | --- |
| Systemanweisungen | Anweisungen, die vor dem Gespräch gesendet werden |
| Temperatur, Top P | Sampling, nur an Modelle gesendet, die Temperatur unterstützen |
| Max. Ausgabe-Token | Die längste Antwort |
| Reasoning-Aufwand | Wie viel ein Reasoning-Modell nachdenkt, von **Ohne** bis **Maximal** |
| Reasoning-Zusammenfassung | Ob das Reasoning eines Modells zusammengefasst wird: **Auto**, **Knapp**, **Detailliert** oder **Aus** |

**Auf Standard zurücksetzen** setzt jeden Parameter außer den Systemanweisungen zurück.

## Was eine Anfrage im Playground kostet

Anfragen im Playground sind echte Anfragen und werden dem Projekt berechnet. Sie nutzen die Anbieter-Schlüssel des Projekts, Ihren Modellzugriff sowie Ihre Limits und die des Projekts. Schlüssel-Limits gelten nicht, weil kein Schlüssel verwendet wird. Die [Anfrageprotokolle](request-logs.md) listen sie mit der Quelle **Playground**.

## Nächste Schritte

- [Modelle und Aliasse](models-and-aliases.md): Fügen Sie die Modelle, die Sie vergleichen möchten, dem Katalog hinzu.
- [API-Schlüssel](api-keys.md): Erstellen Sie einen Schlüssel, um das gewählte Modell aus einer App aufzurufen.
