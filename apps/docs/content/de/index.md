---
title: Einführung
description: Kaeso ist ein Open-Source-KI-Gateway für den eigenen Betrieb.
---

Kaeso ist ein Open-Source-KI-Gateway für den eigenen Betrieb: Anwendungen sprechen mit einer einzigen API, und Anbieter-Schlüssel, Routing und Nutzung bleiben unter eigener Kontrolle.

Anwendungen rufen Kaeso mit den offiziellen SDKs von OpenAI oder Anthropic und einem Kaeso-API-Schlüssel auf. Kaeso prüft den Schlüssel, das Modell und die Limits, sendet die Anfrage mit dem eigenen Anbieter-Schlüssel an den Anbieter und gibt die Antwort im Format zurück, das die Anwendung angefragt hat. Jede Anfrage wird protokolliert und gezählt, sodass sichtbar ist, wer welches Modell genutzt hat und was es gekostet hat.

Neu hier? Der [Schnellstart](get-started/quick-start.md) startet Kaeso mit Docker Compose und sendet in wenigen Minuten eine erste Anfrage.

Diese Dokumentation beschreibt Kaeso 0.1.0.

## Was Kaeso bietet

- **Eine API für jede App.** Die Formate Chat Completions, Responses und Embeddings von OpenAI und das Messages-Format von Anthropic, sodass die SDKs, die Apps schon nutzen, weiter funktionieren. Chat Completions, Responses und Messages können jedes Chat-Modell im Katalog aufrufen, von OpenAI oder Anthropic.
- **Anbieter-Schlüssel bleiben im eigenen Haus.** Apps erhalten Kaeso-API-Schlüssel (`sk-kso-…`), die sich begrenzen, befristen und deaktivieren lassen. Anbieter-Schlüssel werden mit einem Master-Schlüssel verschlüsselt, den nur die eigene Installation besitzt.
- **Organisationen und Projekte.** Mitglieder, Rollen und Rechte, Dienstkonten, ein Modellkatalog und Aliasse pro Projekt.
- **Limits und Budgets.** Anfrage- und Token-Raten, Token- und Ausgabenbudgets für ein Projekt, ein Mitglied oder einen einzelnen Schlüssel.
- **Anfrageprotokolle und Nutzung.** Jede Anfrage mit Status, Tokens, Kosten und Zeiten, und mit ihrem Inhalt, wenn er aufbewahrt werden soll.
- **Eine Konsole.** Eine Weboberfläche für all das, auf Englisch, Deutsch und vereinfachtem Chinesisch.

## Wie die Teile zusammenpassen

```text
app ──▶ gateway ──▶ OpenAI or Anthropic
          ▲
console ──┘
```

| Teil | Was es ist |
| --- | --- |
| Gateway | Der Server, den Anwendungen aufrufen. Er stellt die Modell-API unter `/v1` und die Platform API unter `/platform/v1` bereit und speichert seinen Zustand in PostgreSQL, Anfrageprotokolle in ClickHouse und Zähler in Valkey. |
| Konsole | Die Weboberfläche, unter derselben Domain wie das Gateway bereitgestellt. Sie nutzt nur die Platform API, sodass ein Skript alles kann, was sie kann. |
| KPP | Das Kaeso Provider Protocol: ein kanonisches Format für Modellanfragen und die Codecs, die zwischen ihm und jedem API-Format übersetzen. Das Gateway nutzt es für jede Anfrage. |

Der [Ablauf einer Anfrage](concepts/request-flow.md) erklärt, was mit einer Anfrage auf ihrem Weg durch das Gateway geschieht, und die [Formatübersetzung](concepts/format-translation.md), was KPP mit ihr macht.

## Wo anfangen

| Ziel | Lesen |
| --- | --- |
| Kaeso starten und die erste Anfrage senden | [Schnellstart](get-started/quick-start.md) |
| Modelle aus dem eigenen Code aufrufen | [Modelle aufrufen](get-started/calling-models.md) |
| Kaeso aus Claude Code oder Codex nutzen | [Coding-Tools](get-started/coding-tools.md) |
| Eine Installation planen und betreiben | [Selbst hosten](self-hosting/index.md) |
| Sich in der Konsole zurechtfinden | [Die Konsole nutzen](console/index.md) |
| Die APIs aus Code aufrufen | [API](api/index.md) |
| An Kaeso selbst arbeiten | [Entwicklung](development/index.md) |

Kaeso steht unter der MIT-Lizenz. Der gesamte Quellcode, Gateway, KPP, Konsole, UI-Bibliothek und diese Dokumentation, liegt in einem Repository auf GitHub: [logicplanes/kaeso](https://github.com/logicplanes/kaeso).
