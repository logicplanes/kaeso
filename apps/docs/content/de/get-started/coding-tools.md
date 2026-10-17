---
title: Coding-Tools
description: Kaeso aus Claude Code, Codex und anderen Tools nutzen, die eine API von OpenAI oder Anthropic sprechen.
order: 3
---

Coding-Tools rufen dieselben APIs auf wie jede App; eine Basis-URL und ein API-Schlüssel genügen. Ihre Anfragen erscheinen dann in den Anfrageprotokollen und zählen gegen die Limits und Budgets des Schlüssels.

Jeder Person und jedem Tool einen eigenen Schlüssel geben, damit Nutzung und Limits getrennt bleiben. Im Katalog des Projekts, zu dem der Schlüssel gehört, muss jedes Modell stehen, das das Tool anfragt.

## Claude Code

Claude Code spricht Anthropic Messages. In `~/.claude/settings.json` auf die eigene Installation ausrichten:

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

- `ANTHROPIC_AUTH_TOKEN` sendet den Schlüssel als Bearer-Token, das Kaeso annimmt, und lässt Claude Code seine eigene Anmeldung überspringen.
- Die drei Modellvariablen legen fest, welches Modell die Auswahl `opus`, `sonnet` und `haiku` aufruft. Claude Code nutzt das Haiku-Modell auch für Hintergrundaufgaben, daher müssen alle drei Modelle aus dem Katalog des Projekts nennen.
- Eine Modellvariable kann auch einen Alias des Projekts oder ein OpenAI-Modell nennen; Kaeso übersetzt die Anfragen.

Den Schlüssel in den Benutzereinstellungen oder im Shell-Profil ablegen, nie in der Einstellungsdatei eines Projekts, die committet wird.

> [!TIP]
> Mit einem OpenAI-Modell dahinter sendet Claude Code mitunter Vorabversions-Felder von Anthropic, die Kaeso nicht übersetzen kann; Kaeso lehnt sie mit einem Fehler ab, der sie nennt. `"CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS": "1"` in `env` lässt sie weg.

## Codex

Codex spricht OpenAI Responses. Kaeso in `~/.codex/config.toml` als Modellanbieter eintragen:

```toml
model = "gpt-5-mini"
model_provider = "kaeso"

[model_providers.kaeso]
name = "Kaeso"
base_url = "https://kaeso.example.com/v1"
env_key = "KAESO_API_KEY"
wire_api = "responses"
```

Codex liest den Schlüssel aus der Umgebung:

```bash
export KAESO_API_KEY=sk-kso-…
codex
```

`model` kann jedes Modell im Katalog des Projekts nennen, auch ein Claude-Modell oder einen Alias.

> [!NOTE]
> Kaeso speichert keine Antworten zwischen Runden. Eine Anfrage, die den Anbieter bittet, eine Antwort zu speichern oder fortzusetzen (`store: true`, `previous_response_id`), wird mit einem Fehler abgelehnt, der das sagt.

## Andere Tools

Jedes Tool, in dem sich eine Basis-URL für OpenAI oder Anthropic einstellen lässt, funktioniert genauso:

| Das Tool spricht | Basis-URL | Schlüssel |
| --- | --- | --- |
| OpenAI Chat Completions oder Responses | `https://kaeso.example.com/v1` | Als API-Schlüssel oder Bearer-Token |
| Anthropic Messages | `https://kaeso.example.com` | Als API-Schlüssel (`x-api-key`) oder Bearer-Token |

Die [Modell-API](../api/model-api.md) listet jeden Endpunkt auf, die [Formatübersetzung](../concepts/format-translation.md), was zwischen Formaten übertragen wird und was nicht.
