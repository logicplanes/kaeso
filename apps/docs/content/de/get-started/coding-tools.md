---
title: Coding-Tools
description: Kaeso aus Claude Code, Codex und anderen Tools nutzen, die eine API von OpenAI oder Anthropic sprechen.
order: 3
---

Coding-Tools rufen dieselben APIs auf wie jede App, also funktionieren sie über Kaeso mit einer Basis-URL und einem API-Schlüssel. Die Anfragen jedes Tools erscheinen dann in den Anfrageprotokollen und zählen gegen die Limits und Budgets des Schlüssels.

Für jede Person und jedes Tool einen eigenen Schlüssel erstellen, damit Nutzung und Limits getrennt bleiben. Im Katalog des Projekts, zu dem der Schlüssel gehört, muss jedes Modell stehen, das das Tool anfragt.

## Claude Code

Claude Code spricht das Messages-Format von Anthropic. In `~/.claude/settings.json` auf die eigene Installation ausrichten:

```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "https://kaeso.example.com",
    "ANTHROPIC_AUTH_TOKEN": "sk-kso-…",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "claude-opus-4-8",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "claude-sonnet-5-5",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "claude-haiku-4-5"
  }
}
```

- `ANTHROPIC_AUTH_TOKEN` sendet den Schlüssel als Bearer-Token, das Kaeso akzeptiert. Ist es gesetzt, überspringt Claude Code seine eigene Anmeldung.
- Die drei Modellvariablen legen fest, worauf die Modellauswahl `opus`, `sonnet` und `haiku` verweist. Claude Code nutzt das Haiku-Modell auch für Hintergrundaufgaben, daher alle drei dem Katalog des Projekts hinzufügen oder sie auf Modelle setzen, die er enthält.
- Eine Modellvariable darf auch einen Alias des Projekts oder ein OpenAI-Modell nennen. Kaeso übersetzt die Anfragen.

Den Schlüssel in den Benutzereinstellungen oder im Shell-Profil ablegen, nie in der Einstellungsdatei eines Projekts, die committet wird.

> [!TIP]
> Arbeitet Claude Code mit einem OpenAI-Modell, lehnt Kaeso einige Vorabversions-Felder von Anthropic ab, die es nicht übersetzen kann, und der Fehler sagt das. `"CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS": "1"` zu `env` hinzufügen, um sie wegzulassen.

## Codex

Codex spricht das Responses-Format von OpenAI. Kaeso in `~/.codex/config.toml` als Modellanbieter hinzufügen:

```toml
model = "gpt-5-mini"
model_provider = "kaeso"

[model_providers.kaeso]
name = "Kaeso"
base_url = "https://kaeso.example.com/v1"
env_key = "KAESO_API_KEY"
wire_api = "responses"
```

Dann vor dem Start von Codex den Schlüssel in die Umgebung setzen:

```bash
export KAESO_API_KEY=sk-kso-…
codex
```

`model` darf jedes Modell im Katalog des Projekts nennen, auch ein Claude-Modell oder einen Alias.

> [!NOTE]
> Kaeso ist zustandslos: Es speichert keine Antworten für spätere Runden. Anfragen, die den Anbieter bitten, eine gespeicherte Antwort aufzubewahren oder fortzusetzen (`store: true`, `previous_response_id`), werden mit einem Fehler abgelehnt, der das sagt.

## Andere Tools

Jedes Tool, bei dem sich eine Basis-URL für OpenAI oder Anthropic einstellen lässt, funktioniert genauso:

| Tool spricht | Basis-URL | Schlüssel |
| --- | --- | --- |
| OpenAI Chat Completions oder Responses | `https://kaeso.example.com/v1` | als API-Schlüssel oder Bearer-Token |
| Anthropic Messages | `https://kaeso.example.com` | als API-Schlüssel (`x-api-key`) oder Bearer-Token |

Die [Modell-API](../api/model-api.md) listet jeden Endpunkt auf, und die [Formatübersetzung](../concepts/format-translation.md) zeigt, was zwischen Formaten übertragen werden kann und was nicht.
