---
title: Anbieter-Schlüssel
description: Die OpenAI- und Anthropic-Schlüssel der Organisation verbinden und wählen, welche Projekte sie nutzen.
order: 3
---

Anbieter-Schlüssel sind die Schlüssel, mit denen Kaeso OpenAI und Anthropic aufruft. Sie gehören der Organisation, liegen verschlüsselt unter dem Hauptschlüssel der Installation und werden nach dem Hinzufügen nie wieder angezeigt. Apps sehen sie nie; sie nutzen Kaesos [API-Schlüssel](api-keys.md).

Kaeso 0.1.0 ruft zwei Anbieter auf: OpenAI und Anthropic.

## Schlüssel hinzufügen

**Anbieter** öffnen und **Schlüssel hinzufügen** wählen, oder **Verbinden** neben einem Anbieter ohne Schlüssel.

1. Den Anbieter wählen.
2. Den Schlüssel einfügen: Ein OpenAI-Schlüssel von platform.openai.com beginnt mit `sk-`, ein Anthropic-Schlüssel von console.anthropic.com mit `sk-ant-`. Ihm einen Namen geben, etwa `Production`.
3. Hat der Anbieter schon einen Standardschlüssel, wählen, ob dieser zum Standard wird.

Einen Schlüssel hinzuzufügen erfordert das Recht **Anbieter-Schlüssel anlegen**. Ersetzen, Umbenennen, Zuweisen und Festlegen als Standard erfordern **Anbieter-Schlüssel ändern**, Löschen **Anbieter-Schlüssel löschen**. Hinzufügen und Ändern von Schlüsseln erfordern eine kürzliche Anmeldung.

## Standard und Zuweisungen

Jeder Anbieter hat einen **Standard**-Schlüssel, den jedes Projekt ohne zugewiesenen Schlüssel nutzt. Der erste Schlüssel eines Anbieters wird sein Standard; **Als Standard festlegen** im Menü eines anderen Schlüssels verschiebt ihn.

Um einigen Projekten einen anderen Schlüssel zu geben, im Menü des Schlüssels **Projekten zuweisen** wählen. Diese Projekte nutzen ihn für diesen Anbieter statt des Standards. Ein Projekt nutzt einen Schlüssel pro Anbieter.

## Schlüssel ändern oder entfernen

| Aktion | Wirkung |
| --- | --- |
| Ersetzen | Ein neues Secret für denselben Schlüssel einfügen, etwa nach dem Rotieren beim Anbieter |
| Umbenennen | Ändert den Namen in der Konsole |
| Löschen | Entfernt den Schlüssel; seine Projekte nutzen wieder den Standard. Wird der Standard gelöscht, wird der zuletzt hinzugefügte andere Schlüssel des Anbieters zum Standard; ohne einen solchen schlagen Anfragen an diesen Anbieter fehl. Der Dialog sagt, was zutrifft |

Die Tabelle zeigt für jeden Schlüssel die letzten vier Zeichen, seine Projekte, wer ihn hinzugefügt hat und wann er zuletzt genutzt wurde.
