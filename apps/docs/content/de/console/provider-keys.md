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
3. Fügen Sie den Schlüssel ein und geben Sie ihm einen Namen, etwa `Production`. Der Dialog verlinkt die Seite des Anbieters, auf der Sie Schlüssel erstellen; die Tabelle unten nennt sie ebenfalls.
4. Hat der Anbieter schon einen Standardschlüssel, wählen Sie, ob der neue Schlüssel zum Standard wird.

| Anbieter | Schlüssel erstellen unter | Der Schlüssel beginnt mit |
| --- | --- | --- |
| OpenAI | [platform.openai.com/api-keys](https://platform.openai.com/api-keys) | `sk-` |
| Anthropic | [platform.claude.com/settings/keys](https://platform.claude.com/settings/keys) | `sk-ant-` |
| DeepSeek | [platform.deepseek.com/api_keys](https://platform.deepseek.com/api_keys) | `sk-` |
| SpaceX | [console.x.ai](https://console.x.ai) (xAI-Konsole), unter **API Keys** | `xai-` |
| Together AI | [api.together.ai](https://api.together.ai/settings/projects/~current/api-keys), unter **API Keys** | `tgp_v1_` |
| Groq | [console.groq.com/keys](https://console.groq.com/keys) | `gsk_` |
| OpenRouter | [openrouter.ai/settings/keys](https://openrouter.ai/settings/keys) | `sk-or-v1-` |

OpenRouter leitet an Modelle vieler Anbieter weiter. Seine Modell-IDs nennen den Hersteller des Modells, etwa `anthropic/claude-sonnet-4.5`. Rufen Sie eines als `openrouter/anthropic/claude-sonnet-4.5` auf, damit die Anfrage nie beim Anbieter des Herstellers landet; siehe [Wie eine Anfrage ein Modell nennt](models-and-aliases.md#wie-eine-anfrage-ein-modell-nennt). Kaeso berechnet die Kosten von OpenRouter-Anfragen mit den Preisen des Katalogs, nicht mit dem `usage.cost`, das OpenRouter meldet. OpenRouter listet seine Modelle, ohne den Schlüssel zu prüfen, daher erkennt **Verfügbarkeit prüfen** keinen falschen OpenRouter-Schlüssel; die erste Anfrage damit meldet den abgelehnten Schlüssel.

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
