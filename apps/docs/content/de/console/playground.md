---
title: Playground
description: Senden Sie einen Prompt an bis zu vier Modelle und vergleichen Sie ihre Antworten nebeneinander.
order: 10
---

Der **Playground** sendet einen Prompt an bis zu vier Modelle des Projekts zugleich und zeigt ihre Antworten nebeneinander. Jede Spalte ist ein eigenes Gespräch. Ihn zu nutzen erfordert das Recht **Playground nutzen**, das Admins und Entwickler eines Projekts haben.

Er listet jede Art von Modell, die das Projekt aufrufen kann, und jede Spalte nimmt die Form der Art ihres Modells an: Text, Bilder, Sprache, Soundeffekte, Musik, Transkription, Video oder Entscheidungen. Embeddings-, Reranking- und Echtzeitmodelle fehlen.

## Modelle vergleichen

1. Wählen Sie ein Modell für die erste Spalte und mit **Spalte hinzufügen** jede weitere. Suchen Sie nach Name, ID oder Anbieter, oder beschränken Sie die Liste auf einen Anbieter; die Modelle sind nach Anbieter gruppiert, und die zuletzt gewählten stehen oben. Ein Modell, dessen Anbieter keinen Schlüssel für dieses Projekt hat, können Sie nicht wählen, ebenso wenig eines, das als [Nicht verfügbar](models-and-aliases.md) markiert ist, weil der Anbieter es dem Schlüssel des Projekts nicht anbietet. Auch ein Modell mit dem Hinweis **Braucht einen API-Schlüssel** können Sie nicht wählen: Der Schlüssel des Projekts für seinen Anbieter ist ein Konto, etwa ein [ChatGPT-Konto](provider-keys.md#mit-einem-chatgpt-konto-anmelden), das keine Bilder, Sprachausgabe, Transkriptionen oder Entscheidungen erstellen kann.
2. Senden Sie einen Prompt an alle Modelle. Wenn ein gewähltes Modell Dateien liest, fügt die Schaltfläche zum Anhängen sie hinzu, benannt nach dem, was die Modelle annehmen, etwa **Bilder anhängen**; Sie können sie auch in das Prompt-Feld ziehen oder einfügen. Jedes Modell bekommt die Dateien, die es liest: Bilder und PDFs ein Textmodell, ein Bild ein Bildmodell, das bearbeitet, Audio ein Transkriptionsmodell. Ein Textmodell bekommt sie mit jedem späteren Schritt erneut.
3. Lesen Sie die Antworten. Eine Spalte folgt einer Antwort, während sie eintrifft, bis Sie nach oben scrollen, und folgt wieder, wenn Sie zurück an ihr Ende scrollen. Jede Antwort zeigt die Zeit bis zum ersten Token, die Gesamtzeit, die Token und die Kosten. Eine fehlgeschlagene Antwort zeigt den Grund in ihrer Spalte, und eine, die das Modell abgebrochen hat, sagt warum, etwa **Am Ausgabelimit gestoppt**. **Erneut versuchen** sendet diesen Schritt noch einmal.

**Leeren** leert das Gespräch jeder Spalte; **Rückgängig** in der Meldung holt es zurück.

Die Adresse der Seite enthält die Modelle und Parameter, sodass ein Link denselben Vergleich öffnet. Ein Prompt, den Sie noch nicht gesendet haben, bleibt in Ihrem Browser. Das Gespräch bleibt erhalten, solange die Konsole in diesem Tab geöffnet ist, auch wenn Sie andere Seiten besuchen und zurückkehren. Die Konsole speichert es nie, daher beendet ein Neuladen oder Schließen des Tabs das Gespräch, und der Browser fragt vorher nach.

## Medienmodelle ausprobieren

Ein Prompt geht an jede Spalte, sodass ein Textmodell und ein Bildmodell ihn nebeneinander beantworten können. Jede Medienspalte zeigt anstelle einer Antwort, was ihr Modell erstellt hat:

| Art | Sie senden | Sie bekommen |
| --- | --- | --- |
| Bilder | Einen Prompt und ein Bild zum Ändern, wenn das Modell Bilder bearbeitet | Die Bilder, jedes mit **Herunterladen** |
| Sprache | Den Text, der gesprochen werden soll | Einen Audioplayer mit **Herunterladen** |
| Soundeffekte, Musik | Eine Beschreibung | Einen Audioplayer mit **Herunterladen** |
| Transkription | Eine Audiodatei, angehängt oder mit **Audio aufnehmen** aufgenommen, und eine optionale Notiz | Das Transkript |
| Video | Einen Prompt | Den Fortschritt des Jobs mit **Abbrechen**, dann das Video mit **Herunterladen** |

Ein Video-Job dauert Minuten. Der Playground fragt alle paar Sekunden nach ihm, seltener, je länger er läuft. **Abbrechen** oder **Stoppen** bricht ihn beim Anbieter ab. Nur Sie sehen die Videos, die Sie im Playground starten.

Eine Medienantwort zeigt die Zeit, die sie gebraucht hat, und ihre Kosten. Die Kosten lauten **Unbekannt**, wenn das Modell keinen Preis für das hat, was es erstellt hat, etwa für eine Aufnahme, deren Länge der Browser nicht angibt.

## Entscheidungsmodelle befragen

Ein Entscheidungsmodell beantwortet typisierte Fragen zu dem, was Sie senden, jede Antwort mit ihren Wahrscheinlichkeiten. Eine Spalte mit einem Entscheidungsmodell, etwa Jev oder Gemini Decision 1, stellt ihm die Fragen. Ein Modell, das auch chattet, etwa GPT-6 Luna, chattet, bis es Fragen gibt, und beantwortet ab dann die Fragen.

1. Wählen Sie **Fragen** neben dem Eingabefeld und dann für jede Frage **Frage hinzufügen**. Jede hat einen Typ, einen Namen und Anweisungen:

   | Typ | Das Modell antwortet mit |
   | --- | --- |
   | Ja/Nein | Ja oder Nein, mit dem, was beides bedeutet, wenn Sie es angeben |
   | Auswahl | Einer der Optionen, jede ein Wert mit dem, was er bedeutet |
   | Bewertung | Einer Stufe ab 0, jede mit einem Namen oder dem, was sie bedeutet |
   | Box | Der Stelle auf dem angehängten Bild, an der etwas ist; nur angeboten, wenn jedes gewählte Modell Boxen zeichnet |

   Namen müssen sich unterscheiden; eine Frage ohne Namen wird nummeriert. Die Fragen halten die strengsten Grenzen der gewählten Modelle ein, etwa die 400 Fragen von GPT-6 Luna.
2. Senden Sie, worum es in den Fragen geht: Text oder JSON, wenn der Prompt ein JSON-Objekt oder -Array ist. Hängen Sie PNG- oder JPEG-Bilder an, wenn das Modell Bilder liest; dann kommt zuerst der Text, danach jedes Bild.
3. Lesen Sie die Antworten. Jede Frage zeigt die Antwort, einen Balken mit der Wahrscheinlichkeit jeder Option oder Stufe und die Konfidenz, wenn das Modell sie angibt. Eine Box wird auf das Bild gezeichnet. Eine Frage, die das Modell ablehnte, sagt das in ihrer Zeile, und eine abgelehnte Anfrage zeigt den Grund in der Spalte.

Ein Chatmodell im selben Vergleich erhält denselben Text und dieselben Bilder als Prompt. Die Fragen bleiben, solange die Konsole in diesem Tab offen ist, und jede Runde behält die Fragen, die sie stellte. Eine Entscheidung zeigt ihre Dauer, ihre Tokens und ihre Kosten.

## Parameter festlegen

**Parameter** legt fest, was jede Spalte mitsendet:

| Parameter | Wirkung |
| --- | --- |
| Systemanweisungen | Anweisungen, die vor dem Gespräch gesendet werden |
| Temperatur, Top P | Sampling, nur an Modelle gesendet, die Temperatur unterstützen |
| Max. Ausgabe-Token | Die längste Antwort |
| Reasoning-Aufwand | Wie viel ein Reasoning-Modell nachdenkt, von **Ohne** bis **Maximal** |
| Reasoning-Zusammenfassung | Ob das Reasoning eines Modells zusammengefasst wird: **Auto**, **Knapp**, **Detailliert** oder **Aus** |

Für Medienmodelle zeigt **Parameter** nur, was die gewählten Modelle annehmen:

| Parameter | Wirkung |
| --- | --- |
| Bildgröße, Bilder pro Prompt, Bildqualität | Die Größe, die Anzahl und die Qualität der Bilder eines Bildmodells |
| Stimme | Die Stimme, in der ein Sprachmodell spricht |
| Länge in Sekunden | Wie lang ein Soundeffekt oder ein Musikstück ist |
| Transkript | **Wie gesprochen** oder **Auf Englisch** für ein Transkriptionsmodell, das übersetzt |

**Auf Standard zurücksetzen** setzt jeden Parameter außer den Systemanweisungen zurück.

## Was eine Anfrage im Playground kostet

Anfragen im Playground sind echte Anfragen und werden dem Projekt berechnet. Sie nutzen die Anbieter-Schlüssel des Projekts, Ihren Modellzugriff sowie Ihre Limits und die des Projekts. Schlüssel-Limits gelten nicht, weil kein Schlüssel verwendet wird. Die [Anfrageprotokolle](request-logs.md) listen sie mit der Quelle **Playground**.

## Nächste Schritte

- [Modelle und Aliasse](models-and-aliases.md): Fügen Sie die Modelle, die Sie vergleichen möchten, dem Katalog hinzu.
- [API-Schlüssel](api-keys.md): Erstellen Sie einen Schlüssel, um das gewählte Modell aus einer App aufzurufen.
