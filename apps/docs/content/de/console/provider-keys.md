---
title: Anbieter-Schlüssel
description: Verbinden Sie die Schlüssel Ihrer Organisation für Modellanbieter und wählen Sie, welche Projekte welchen nutzen.
order: 3
---

Anbieter-Schlüssel sind die Schlüssel Ihrer Organisation für die Modellanbieter, die Kaeso aufruft. Kaeso speichert sie verschlüsselt mit dem Hauptschlüssel der Installation und zeigt sie nie wieder an. Apps sehen sie nie: Sie rufen Kaeso stattdessen mit [API-Schlüsseln](api-keys.md) auf.

## Schlüssel hinzufügen

**Anbieter** listet die Anbieter, für die Ihre Organisation Schlüssel hat. Solange sie keinen hat, erklärt die Seite Anbieter-Schlüssel und bietet **Anbieter-Schlüssel hinzufügen** an.

1. Wählen Sie unter **Anbieter** **Schlüssel hinzufügen**.
2. **Für welchen Anbieter ist der Schlüssel?** Suchen Sie den Anbieter und wählen Sie ihn. Anbieter, die schon Schlüssel haben, stehen oben, mit der Anzahl ihrer Schlüssel.
3. Fügen Sie den Schlüssel ein und geben Sie ihm einen Namen, etwa `Production`. Die Tabelle unten nennt, woher die Schlüssel jedes Anbieters kommen.
4. Hat der Anbieter schon einen Standardschlüssel, wählen Sie, ob der neue Schlüssel zum Standard wird.

| Anbieter | Schlüssel erstellen unter | Der Schlüssel beginnt mit |
| --- | --- | --- |
| OpenAI | platform.openai.com | `sk-` |
| Anthropic | console.anthropic.com | `sk-ant-` |
| DeepSeek | platform.deepseek.com | `sk-` |

Jede Aktion an Anbieter-Schlüsseln braucht ihr eigenes Recht:

| Aktion | Recht |
| --- | --- |
| Schlüssel hinzufügen | **Anbieter-Schlüssel hinzufügen** |
| Schlüssel ersetzen, umbenennen, zuweisen oder zum Standard machen | **Anbieter-Schlüssel ändern** |
| Schlüssel löschen | **Anbieter-Schlüssel löschen** |

Jede Aktion außer dem Löschen erfordert zusätzlich eine kürzliche Anmeldung.

## Schlüssel eines Projekts wählen

Jeder Anbieter hat einen Standardschlüssel, den jedes Projekt ohne zugewiesenen Schlüssel nutzt. Der erste Schlüssel, den Sie für einen Anbieter hinzufügen, wird sein Standard. Um den Standard zu verlegen, wählen Sie **Als Standard festlegen** im Menü eines anderen Schlüssels.

Um einigen Projekten einen anderen Schlüssel zu geben, wählen Sie **Projekten zuweisen** im Menü dieses Schlüssels und wählen Sie die Projekte. Sie nutzen dann für diesen Anbieter diesen Schlüssel statt des Standards. Ein Projekt nutzt je Anbieter einen Schlüssel.

## Schlüssel ersetzen, umbenennen oder löschen

Die Liste zeigt zu jedem Schlüssel seinen Namen, die letzten vier Zeichen, die Projekte, die ihn nutzen, wer ihn hinzugefügt hat und wann er zuletzt genutzt wurde. Das Menü eines Schlüssels hat diese Aktionen:

| Aktion | Wirkung |
| --- | --- |
| Ersetzen | Nimmt ein neues Secret für denselben Schlüssel an, etwa nachdem Sie ihn beim Anbieter rotiert haben |
| Umbenennen | Ändert den Namen, den die Konsole zeigt |
| Löschen | Entfernt den Schlüssel. Seine Projekte nutzen wieder den Standard |

Wenn Sie den Standard löschen, wird der zuletzt hinzugefügte andere Schlüssel des Anbieters zum Standard. Gibt es keinen, schlagen Anfragen an diesen Anbieter fehl. Der Dialog sagt Ihnen, was zutrifft.

> [!NOTE]
> Der Schlüssel eines Projekts bestimmt, welche Modelle der Anbieter ihm bereitstellt. Wenn Sie einen Schlüssel hinzufügen, ersetzen, zuweisen, löschen oder zum Standard machen, setzt Kaeso die [Verfügbarkeit](models-and-aliases.md#verfügbarkeit) der Modelle dieses Anbieters in jedem Projekt auf **Nicht geprüft** zurück.

## Nächste Schritte

- [Modelle und Aliasse](models-and-aliases.md): Fügen Sie die Modelle hinzu, die ein Projekt aufrufen darf.
- [API-Schlüssel](api-keys.md): Erstellen Sie die Schlüssel, mit denen Ihre Apps Kaeso aufrufen.
