---
title: Coding-Tools
description: Claude Code, Codex, Gemini CLI und andere Coding-Tools auf Ihre Installation ausrichten.
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

Claude Code berechnet Token mit seiner eigenen Tabelle von Claude-Preisen, daher sind `/cost` und `total_cost_usd` für andere Modelle falsch. Wo Anfrageprotokolle aktiv sind, zeigen sie die Kosten, die Kaeso aus den Preisen des Modells ermittelt ([Anfrageprotokolle](../console/request-logs.md)).

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
> Kaeso speichert keine Antworten zwischen Zügen. Eine Anfrage, die den Anbieter bittet, eine Antwort zu speichern oder fortzusetzen (`store: true`, `previous_response_id`), lehnt Kaeso mit einem Fehler ab, der das sagt, außer das Projekt erlaubt [beim Anbieter gespeicherte Antworten](../console/organizations-and-projects.md#gespeicherte-antworten).

## Gemini CLI einrichten

Gemini CLI spricht die Gemini API. Weisen Sie es zuerst in `~/.gemini/settings.json` an, sich mit einem Schlüssel statt mit einem Google-Konto anzumelden ([Authentifizierung](https://geminicli.com/docs/get-started/authentication)); sonst gewinnt eine gespeicherte Google-Anmeldung:

```json
{ "security": { "auth": { "selectedType": "gemini-api-key" } } }
```

Setzen Sie dann die Basis-URL ohne Version, da Gemini CLI `/v1beta` selbst anhängt, und wählen Sie das Modell mit `-m`:

```bash
export GOOGLE_GEMINI_BASE_URL=https://kaeso.example.com
export GEMINI_API_KEY=sk-kso-…
gemini -m claude-sonnet-5-5
```

Gemini CLI sendet `topK` mit jeder Anfrage. Modelle ohne Top-k-Sampling, etwa GPT und DeepSeek, bekommen die Anfrage ohne es, und der Header `kaeso-left-out` der Antwort nennt es.

## Qwen Code einrichten

[Qwen Code](https://github.com/QwenLM/qwen-code) spricht OpenAI Chat Completions. Wählen Sie OpenAI als Anmeldung und das Modell mit `-m`:

```bash
export OPENAI_BASE_URL=https://kaeso.example.com/v1
export OPENAI_API_KEY=sk-kso-…
qwen --auth-type openai -m claude-sonnet-5-5
```

## OpenCode einrichten

[OpenCode](https://opencode.ai/docs/providers/) nimmt Kaeso als Anbieter. Führen Sie Chat-Modelle mit einem leeren Eintrag auf. Entscheidungsmodelle ruft OpenCode im System-One-Format auf, das Kaeso unter `/v1/systemone` bedient, daher kennzeichnen Sie jedes [Entscheidungsmodell](../api/decisions.md) mit `type: "decision"`:

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "providers": {
    "kaeso": {
      "name": "Kaeso",
      "env": ["KAESO_API_KEY"],
      "package": "@opencode/ai/providers/openai-compatible",
      "settings": { "baseURL": "https://kaeso.example.com/v1" },
      "models": {
        "claude-sonnet-5-5": {},
        "jev-latest": {
          "type": "decision",
          "compatibility": { "mediaInput": "assets" },
        },
      },
    },
  },
}
```

Der Schlüssel des Modells ist sein Name in Ihrem Kaeso-Projekt. Mit `mediaInput: "assets"` sendet OpenCode Bilder als System-One-Assets, die Form, die Kaeso liest; Kaeso legt sie dorthin, wo der Anbieter des Modells sie erwartet.

## Andere Tools einrichten

Jedes Tool, in dem Sie eine Basis-URL für OpenAI oder Anthropic setzen können, funktioniert genauso:

| Das Tool spricht | Basis-URL | Den Schlüssel senden als |
| --- | --- | --- |
| OpenAI Chat Completions oder Responses | `https://kaeso.example.com/v1` | API-Schlüssel oder Bearer-Token |
| Anthropic Messages | `https://kaeso.example.com` | API-Schlüssel (`x-api-key`) oder Bearer-Token |

Manche Tools brauchen mehr als die Basis-URL:

| Tool | Was Sie setzen |
| --- | --- |
| [Crush](https://github.com/charmbracelet/crush#custom-providers) | Einen Anbieter mit `type: "openai-compat"` und `base_url: "https://kaeso.example.com/v1"` oder mit `type: "anthropic"` und `https://kaeso.example.com`. Geben Sie jedem Modell ein `context_window` und ein `default_max_tokens`. |
| [Goose](https://goose-docs.ai/docs/getting-started/providers/) | `OPENAI_HOST=https://kaeso.example.com` ohne `/v1`: Goose hängt `v1/chat/completions` selbst an. Für Anthropic Messages stattdessen `ANTHROPIC_HOST`. |
| [Aider](https://aider.chat/docs/llms/openai-compat.html) | `--model openai/<model>` mit `OPENAI_API_BASE=https://kaeso.example.com/v1` oder `--model anthropic/<model>` mit `ANTHROPIC_API_BASE=https://kaeso.example.com`. Fügen Sie `--no-show-model-warnings` für Modelle hinzu, die Aider nicht kennt. Seine Token-Zahlen sind eigene Schätzungen. |
| [Continue](https://docs.continue.dev/customize/model-providers/top-level/openai) | Ein Modell mit `provider: openai` und `apiBase: https://kaeso.example.com/v1` oder mit `provider: anthropic` und `apiBase: https://kaeso.example.com/v1/`. |
| [Open WebUI](https://docs.openwebui.com/reference/env-configuration) | `OPENAI_API_BASE_URL=https://kaeso.example.com/v1` und `OPENAI_API_KEY`. Für seine Dokumentsuche über Kaeso `RAG_EMBEDDING_ENGINE=openai`, `RAG_OPENAI_API_BASE_URL=https://kaeso.example.com/v1` (es übernimmt nicht die der Chat-Verbindung), `RAG_OPENAI_API_KEY` und ein Embedding-Modell des Projekts in `RAG_EMBEDDING_MODEL`. |

## Nächste Schritte

- [API-Schlüssel](../console/api-keys.md): Beschränken Sie den Schlüssel eines Tools auf die Modelle, die es braucht, oder geben Sie ihm ein Ablaufdatum.
- [Limits und Budgets](../console/limits-and-budgets.md): Begrenzen Sie, wie viele Token oder wie viel Geld ein Tool verbrauchen kann.
- [Formatübersetzung](../concepts/format-translation.md): Sehen Sie, was erhalten bleibt, wenn ein Tool das Modell eines anderen Anbieters aufruft.
- [MCP-Server](../api/mcp.md): Lassen Sie ein Coding-Tool Ihre Installation verwalten, mit den Rechten, die Sie wählen.
