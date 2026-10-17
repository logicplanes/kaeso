---
title: Anbieter-Schlüssel
description: Die OpenAI- und Anthropic-Schlüssel der Organisation verbinden und wählen, welche Projekte sie nutzen.
order: 3
---

Anbieter-Schlüssel sind die Schlüssel der Organisation für OpenAI und Anthropic, die beiden Anbieter, die Kaeso aufruft. Kaeso speichert sie verschlüsselt mit dem Hauptschlüssel der Installation und zeigt sie nie wieder an; Apps bekommen sie nie zu sehen und rufen Kaeso stattdessen mit [API-Schlüsseln](api-keys.md) auf.

## Schlüssel hinzufügen

Unter **Anbieter** **Schlüssel hinzufügen** wählen, oder **Verbinden** neben einem Anbieter ohne Schlüssel.

1. **Für welchen Anbieter ist der Schlüssel?**
2. Den Schlüssel einfügen und benennen, etwa `Production`. Ein OpenAI-Schlüssel von platform.openai.com beginnt mit `sk-`, ein Anthropic-Schlüssel von console.anthropic.com mit `sk-ant-`.
3. Hat der Anbieter schon einen Standardschlüssel, wählen, ob dieser neue zum Standard wird.

| Aktion | Recht |
| --- | --- |
| Schlüssel hinzufügen | **Anbieter-Schlüssel hinzufügen** |
| Schlüssel ersetzen, umbenennen, zuweisen oder zum Standard machen | **Anbieter-Schlüssel ändern** |
| Schlüssel löschen | **Anbieter-Schlüssel löschen** |

Jede Aktion außer dem Löschen erfordert zusätzlich eine kürzliche Anmeldung.

## Standard und Zuweisungen

Jeder Anbieter hat einen **Standardschlüssel**, den jedes Projekt ohne zugewiesenen Schlüssel nutzt. Der erste Schlüssel eines Anbieters wird sein Standard; **Als Standard festlegen** im Menü eines anderen Schlüssels verlegt ihn.

**Projekten zuweisen** im Menü eines Schlüssels lässt die gewählten Projekte für diesen Anbieter diesen Schlüssel statt des Standards nutzen. Ein Projekt nutzt je Anbieter einen Schlüssel.

## Schlüssel ändern oder entfernen

Die Liste zeigt zu jedem Schlüssel Name, die letzten vier Zeichen, die Projekte, die ihn nutzen, wer ihn hinzugefügt hat und wann er zuletzt genutzt wurde. Das Menü eines Schlüssels hat diese Aktionen:

| Aktion | Wirkung |
| --- | --- |
| Ersetzen | Nimmt ein neues Secret für denselben Schlüssel an, etwa nachdem er beim Anbieter rotiert wurde |
| Umbenennen | Ändert den Namen in der Konsole |
| Löschen | Entfernt den Schlüssel; seine Projekte nutzen wieder den Standard. War er der Standard, wird der zuletzt hinzugefügte andere Schlüssel des Anbieters zum Standard; gibt es keinen, schlagen Anfragen an diesen Anbieter fehl. Der Dialog sagt, was zutrifft |

Welchen Schlüssel ein Projekt nutzt, bestimmt, welche Modelle der Anbieter ihm bereitstellt. Hinzufügen, Ersetzen, Zuweisen, zum Standard Machen oder Löschen eines Schlüssels setzt deshalb die [Verfügbarkeit](models-and-aliases.md#verfügbarkeit) der Modelle dieses Anbieters in jedem Projekt der Organisation auf **Nicht geprüft** zurück; **Verfügbarkeit prüfen** auf der Seite **Modelle** eines Projekts fragt den Anbieter erneut.
