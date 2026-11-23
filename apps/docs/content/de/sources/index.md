---
title: Quellen verbinden
description: Fügen Sie einem Projekt eine Quelle hinzu, verbinden Sie ihren Client und verfolgen Sie die gemeldeten Anfragen und Tool-Aufrufe.
order: 4
---

Eine Quelle lässt einen Client, der seine Anbieter direkt aufruft, etwa Claude Code, Codex oder einen LiteLLM-Proxy, diese Anfragen an ein Projekt melden. Sie erscheinen dann in den Anfrageprotokollen, der Nutzung, den Kosten und den Limits des Projekts, markiert als beobachtet ([Quellen](../concepts/sources.md) erklärt, wie Kaeso sie zählt). Für **Quellen** brauchen Sie `sources.view`, zum Hinzufügen und Ändern von Quellen `sources.manage`.

## Quelle hinzufügen

1. Öffnen Sie **Quellen** im Projekt und wählen Sie **Quelle hinzufügen**.
2. Wählen Sie, was an Kaeso meldet, etwa **Claude Code**, und wählen Sie **Weiter**.
3. Benennen Sie die Quelle, etwa `Claude Code, Adas Laptop`, und wählen Sie, für wen ihre Anfragen zählen: für Sie oder für eines der Dienstkonten des Projekts.
4. Schalten Sie **Mitglieder per E-Mail zuordnen** ein, damit ein Ereignis mit der E-Mail-Adresse eines Mitglieds für dieses Mitglied zählt; es ist aus, bis Sie es einschalten, da sonst jeder mit dem Schlüssel jedes Mitglied nennen könnte. Schalten Sie **Prompts und Antworten behalten** ein, um die Inhalte zu behalten, die der Client sendet.
5. Wählen Sie **Quelle hinzufügen**.

Kaeso zeigt den Schlüssel der Quelle einmal, mit den genauen Zeilen, die ihren Client verbinden: Umgebungsvariablen, eine Einstellungsdatei, einen Hook oder die Konfiguration eines Gateways. Kopieren Sie den Schlüssel sofort, denn Kaeso behält nur einen Hash davon. Tragen Sie die Zeilen ein und nutzen Sie den Client einmal. Der Dialog wartet auf das erste Ereignis und zeigt dann **Verbunden** und was ankam, oder warum Kaeso es abgelehnt hat.

## Quelle verfolgen

**Quellen** listet jede Quelle mit ihrem Client, ihrer letzten Meldung, Ablehnungen der letzten 14 Tage, ihren Ereignissen pro Tag und ihrem Status:

| Status | Bedeutung |
| --- | --- |
| **Wartet** | Noch kein Ereignis |
| **Verbunden** | Sie hat innerhalb des letzten Tages gemeldet |
| **Ruhend** | Sie hat davor gemeldet |
| **Deaktiviert** | Kaeso lehnt ab, was sie sendet, bis sie wieder eingeschaltet ist |
| **Archiviert** | Ihr Schlüssel funktioniert nicht mehr; ihre Anfragen bleiben |

Öffnen Sie eine Quelle, um ihre Aktivität pro Tag zu sehen, mit den ausgelassenen Duplikaten und dem Abgelehnten. Ihre Detailansicht zeigt auch den Grund der letzten Ablehnung und ihre Einrichtungszeilen, mit einem Platzhalter für den Schlüssel. **Anfragen ansehen** öffnet das Anfrageprotokoll nur mit ihren Anfragen.

Im Anfrageprotokoll trägt eine beobachtete Anfrage die Markierung **Beobachtet** und nennt ihre Quelle, wo sonst ein Schlüssel stünde. Der Filter **Gemeldet von** zeigt die Anfragen einer Quelle, der Filter **Quelle** alle beobachteten. **Analysen** schlüsselt die Nutzung **Nach Quelle** auf, die eigenen Anfragen des Gateways getrennt von denen jeder Quelle, und das Tool-Aufruf-Protokoll zeigt die MCP-Aufrufe, die eine Quelle gemeldet hat.

## Quelle ändern

In der Detailansicht einer Quelle können Sie:

- Sie umbenennen oder ändern, für wen ihre Anfragen zählen.
- **Mitglieder per E-Mail zuordnen** oder **Prompts und Antworten behalten** ein- oder ausschalten.
- **Ihre Ereignisse annehmen** ausschalten, damit Kaeso ablehnt, was sie sendet, etwa während Sie einen Client umziehen.
- **Neuen Schlüssel geben** wählen. Der alte Schlüssel gilt sofort nicht mehr, also tragen Sie den neuen in die Einrichtung des Clients ein.
- **Quelle archivieren** wählen. Archivieren ist endgültig: Der Schlüssel funktioniert nicht mehr, und die Anfragen der Quelle bleiben in den Protokollen.

## Nächste Schritte

- [Claude Code](claude-code.md): Zählen Sie die Anfragen und MCP-Tool-Aufrufe von Claude Code.
- [Codex](codex.md): Zählen Sie die Anfragen und MCP-Tool-Aufrufe der Codex-CLI.
- [Gemini CLI](gemini-cli.md): Zählen Sie die Anfragen und MCP-Tool-Aufrufe von Gemini CLI.
- [Antigravity](antigravity.md): Zählen Sie die Modellaufrufe von Antigravity mit einem Hook-Skript.
- [LiteLLM](litellm.md): Zählen Sie die Anfragen, die ein LiteLLM-Proxy bedient.
- [OpenRouter](openrouter.md): Zählen Sie Ihre OpenRouter-Anfragen mit Broadcast.
- [OpenTelemetry](opentelemetry.md): Zählen Sie die Anfragen jeder App, die GenAI-Telemetrie exportiert.
- [HTTP](http.md): Senden Sie Anfragen und Tool-Aufrufe aus eigenen Skripten.
