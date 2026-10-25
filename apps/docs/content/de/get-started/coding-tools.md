---
title: Coding-Tools
description: Claude Code, Codex und andere Coding-Tools auf Ihre Installation ausrichten.
order: 3
---

Coding-Tools rufen dieselben APIs auf wie jede App, daher brauchen sie nur eine Basis-URL und einen API-Schlüssel. Ihre Anfragen erscheinen dann in den Anfrageprotokollen und zählen gegen die Limits und Budgets des Schlüssels.

## Einen Schlüssel pro Tool erstellen

Geben Sie jeder Person und jedem Tool einen eigenen [API-Schlüssel](../console/api-keys.md), damit Nutzung und Limits getrennt bleiben. Im Katalog des Projekts, zu dem der Schlüssel gehört, muss jedes Modell stehen, das das Tool anfragt.

## Claude Code einrichten

Claude Code spricht Anthropic Messages. Richten Sie es in `~/.claude/settings.json` auf Ihre Installation aus:

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "https://kaeso.example.com",
    "ANTHROPIC_AUTH_TOKEN": "sk-kso-…",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "claude-opus-5-5",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "claude-sonnet-5-5",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "claude-haiku-4-5"
  }
}
```

- `ANTHROPIC_AUTH_TOKEN` sendet den Schlüssel als Bearer-Token, das Kaeso annimmt. Claude Code überspringt dann seine eigene Anmeldung.
- Die drei Modellvariablen legen fest, was die Auswahl `opus`, `sonnet` und `haiku` aufruft. Claude Code nutzt das Haiku-Modell auch für Hintergrundaufgaben, daher müssen alle drei im Katalog des Projekts stehen.
- Eine Modellvariable kann auch einen Alias des Projekts oder ein OpenAI-Modell nennen. Kaeso übersetzt die Anfragen.

Mit einem OpenAI-Modell dahinter sendet Claude Code mitunter Vorabversions-Felder von Anthropic, die Kaeso nicht übersetzen kann. Kaeso antwortet dann mit einem Fehler, der sie nennt. Fügen Sie `"CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS": "1"` zu `env` hinzu, um sie wegzulassen.

`WebSearch` in Claude Code nutzt die Websuche von GPT- und Grok-Modellen. Modelle, die über Chat Completions bereitgestellt werden, etwa Mistral, haben keine Websuche und beantworten `WebSearch` daher ohne zu suchen.

> [!WARNING]
> Legen Sie den Schlüssel in Ihren Benutzereinstellungen oder Ihrem Shell-Profil ab. Die Einstellungsdatei eines Projekts wird committet, und jeder, der das Repository lesen kann, könnte den Schlüssel nutzen.

## Codex einrichten

Codex spricht OpenAI Responses. Tragen Sie Kaeso in `~/.codex/config.toml` als Modellanbieter ein:

```toml
model = "gpt-5-mini"
model_provider = "kaeso"

[model_providers.kaeso]
name = "Kaeso"
base_url = "https://kaeso.example.com/v1"
env_key = "KAESO_API_KEY"
wire_api = "responses"
```

`model` kann jedes Modell im Katalog des Projekts nennen, auch ein Claude-Modell oder einen Alias. Codex liest den Schlüssel aus der Umgebung. Setzen Sie ihn also, bevor Sie Codex starten:

```bash
export KAESO_API_KEY=sk-kso-…
codex
```

Codex sendet bei jedem Zug Teile, die nur OpenAI kennt. Kaeso übersetzt sie für andere Modelle: Es macht aus den Tool-Gruppen und MCP-Tools von Codex einfache Tools und aus seiner Websuche die eigene Websuche des Modells. Ein Thread kann das Modell wechseln: Der Verlauf der Tools eines GPT-Modells bleibt erhalten.

Modelle ohne Websuche, etwa Mistral, bekommen die Anfrage ohne sie. Claude und Grok suchen nur live, während der Standard von Codex, `web_search = "cached"`, OpenAIs Cache von Seiten verlangt. Mit diesem Standard bekommen auch sie keine Websuche. Setzen Sie `web_search = "live"`, damit sie suchen können.

> [!NOTE]
> Kaeso speichert keine Antworten zwischen Zügen. Eine Anfrage, die den Anbieter bittet, eine Antwort zu speichern oder fortzusetzen (`store: true`, `previous_response_id`), lehnt Kaeso mit einem Fehler ab, der das sagt.

## Andere Tools einrichten

Jedes Tool, in dem Sie eine Basis-URL für OpenAI oder Anthropic setzen können, funktioniert genauso:

| Das Tool spricht | Basis-URL | Den Schlüssel senden als |
| --- | --- | --- |
| OpenAI Chat Completions oder Responses | `https://kaeso.example.com/v1` | API-Schlüssel oder Bearer-Token |
| Anthropic Messages | `https://kaeso.example.com` | API-Schlüssel (`x-api-key`) oder Bearer-Token |

## Nächste Schritte

- [API-Schlüssel](../console/api-keys.md): Beschränken Sie den Schlüssel eines Tools auf die Modelle, die es braucht, oder geben Sie ihm ein Ablaufdatum.
- [Limits und Budgets](../console/limits-and-budgets.md): Begrenzen Sie, wie viele Token oder wie viel Geld ein Tool verbrauchen kann.
- [Formatübersetzung](../concepts/format-translation.md): Sehen Sie, was erhalten bleibt, wenn ein Tool das Modell eines anderen Anbieters aufruft.
- [MCP-Server](../api/mcp.md): Lassen Sie ein Coding-Tool Ihre Installation verwalten, mit den Rechten, die Sie wählen.
