---
title: Catalogue format
description: Every field of the KPMR catalogue, format 1, and how the gateway reads it.
order: 2
---

The catalogue is one JSON document in Kaeso's own format, served as `application/vnd.kaeso.kpmr+json; version=1`. This page lists every field of format 1. Here is a shortened catalogue with one provider and one model:

```json
{
  "format": 1,
  "revision": "2026-11-21T02:59:02Z-efb348278140",
  "generated_at": "2026-11-21T02:59:02Z",
  "refresh_seconds": 3600,
  "provider_ids": ["302ai", "alibaba", "anthropic"],
  "providers": {
    "anthropic": {
      "id": "anthropic",
      "name": "Anthropic",
      "docs_url": "https://docs.anthropic.com/en/docs/about-claude/models",
      "api_url": null,
      "env": ["ANTHROPIC_API_KEY"],
      "lines": {
        "claude-opus-latest": {
          "model": "claude-opus-5-5",
          "since": "2026-09-22",
          "previous": "claude-opus-5"
        }
      },
      "models": {
        "claude-opus-5-5": {
          "id": "claude-opus-5-5",
          "name": "Claude Opus 5.5",
          "family": "claude-opus",
          "kind": "generation",
          "status": "active",
          "release_date": "2026-09-22",
          "modalities": {
            "input": ["text", "image", "pdf"],
            "output": ["text"]
          },
          "limits": { "context": 1000000, "input": null, "output": 128000 },
          "pricing": {
            "unit": "usd_per_million_tokens",
            "input": "4",
            "output": "20",
            "cache_read": "0.2",
            "cache_write": "5",
            "cache_write_1h": "8",
            "tiers": []
          },
          "reasoning": {
            "efforts": ["low", "medium", "high", "xhigh", "max"],
            "adaptive_thinking": true,
            "thinking_binding": true
          },
          "decisions": null,
          "lifecycle": { "shutdown_on": null, "replacement": null },
          "gateway": { "egress_dialect": null, "router": false }
        }
      }
    }
  }
}
```

Four rules hold for the whole document:

- **Every key is present.** An unknown value is `null`, never `false`, so a reader can tell "unknown" from "no". The example above leaves keys out only to stay short.
- **Prices are exact decimal strings** in US dollars, never floating-point numbers. Add them up with a decimal library.
- **Providers and models are maps** keyed by id and sorted.
- **Unknown keys are ignored.** Format 1 grows by adding fields, and a reader ignores what it doesn't know. A change that would break a reader becomes format 2, and KPMR serves both while gateways move.

## Document

| Field | Meaning |
| --- | --- |
| `format` | The format version, `1` |
| `revision` | The time the content was built and a hash of it. It changes whenever the content does |
| `generated_at` | When this content was first published, in UTC |
| `refresh_seconds` | How many seconds to wait before asking again |
| `sources` | Where the data comes from and under which license, and the registry's `commit` and number of `entries` |
| `provider_ids` | The id of every provider KPMR knows, also when you asked for only some. A [custom provider](../console/custom-providers.md) can't take one of them |
| `providers` | The providers you asked for, keyed by id. An id KPMR doesn't know is left out |

## Providers

| Field | Meaning |
| --- | --- |
| `id` | The provider id, as Kaeso names it, such as `anthropic` ([supported providers](../concepts/supported-providers.md)) |
| `name` | The provider's name |
| `docs_url` | The provider's model documentation |
| `api_url` | The base address of an OpenAI-compatible API, where the provider has one |
| `env` | The environment variables its SDKs read the key from |
| `models` | The provider's models, keyed by the id its API takes |
| `lines` | The provider's model lines, keyed by name, such as `claude-opus-latest`. Empty when it has none |

### Lines

Each entry of `lines` points a name at one model of the provider ([retirements and model lines](lifecycle-and-lines.md#model-lines)):

| Field | Meaning |
| --- | --- |
| `model` | The model id the line points at now |
| `since` | When that model was released and took the line; `null` when unknown |
| `previous` | The model the line pointed at before; `null` for a line that has had one model |

## Models

| Field | Meaning |
| --- | --- |
| `id` | The id to send to the provider's API, the same as the model's key |
| `name` | The display name |
| `description` | One sentence on what the model is for |
| `family` | The model family, such as `claude-sonnet` |
| `kind` | What the model does: `generation`, `embedding`, `decision`, `image`, `video`, `speech`, `transcription`, `realtime`, `reranking`, `music` or `sound_effects`. Read a kind you don't know as unknown |
| `status` | `active`, `preview`, `deprecated` or `retired`; it follows the `lifecycle` dates, and a model can be `deprecated` before any date is announced |
| `release_date` | When the model was released, `YYYY-MM-DD`, or `YYYY-MM` where only the month is known |
| `last_updated` | When the model was last updated, in the same form |
| `knowledge` | The training data cutoff, `YYYY-MM` or `YYYY-MM-DD` |
| `open_weights` | Whether the weights are published |
| `modalities` | `input` and `output`, each a list of `text`, `image`, `audio`, `video` and `pdf`. `output` is empty for a model that only decides |
| `limits` | `context`, `input` and `output`, in tokens |
| `pricing` | The model's prices ([prices](#prices)) |
| `capabilities` | What the model accepts ([capabilities](#capabilities)) |
| `reasoning` | `null` for a model that doesn't reason; otherwise how it reasons ([reasoning](#reasoning)) |
| `decisions` | The questions a decision model answers; `null` for a model that doesn't decide ([decisions](#decisions)) |
| `lifecycle` | When the provider retires the model ([lifecycle](#lifecycle)) |
| `gateway` | How a gateway sends the model's requests ([gateway](#gateway)) |

### Capabilities

| Field | `true` when the model |
| --- | --- |
| `tool_call` | Calls tools |
| `forced_tool_choice` | Takes a `tool_choice` that forces a tool call. With `false`, Kaeso refuses a forced choice and asks for `auto` |
| `structured_output` | Answers in a JSON schema the request gives |
| `temperature` | Takes `temperature` |
| `attachments` | Takes files, such as images or PDFs |
| `reasoning` | Reasons before it answers |
| `interleaved_reasoning` | Reasons between tool calls in one answer |

### Reasoning

| Field | Meaning |
| --- | --- |
| `efforts` | The effort levels the model accepts, such as `low` and `high`; `[]` for none, `null` when unknown |
| `adaptive_thinking` | The model accepts adaptive thinking, where it decides how long to think |
| `thinking_binding` | Thinking blocks must stay with the turn they belong to |

### Decisions

A decision model answers typed questions with probabilities instead of writing text ([Decisions API](../api/decisions.md)). A model that only decides has `kind: "decision"`, such as `gemini-decision-1`: its output modalities are empty, so a gateway that doesn't know decisions never offers it for chat. A model that also writes text, such as `gpt-6-luna`, keeps its kind and has `decisions` too. Decision models cost their input tokens; their output price is `0`.

| Field | Meaning |
| --- | --- |
| `question_types` | `predicate` (yes or no), `choice` (one of several options), `score` (a level on an ordered scale) or `box` (where something is in an image) |
| `max_questions` | The most questions one request may ask |
| `max_options` | The most options a `choice` question may have |
| `max_levels` | The most levels a `score` question may have |
| `max_images` | The most images one request may send |

Each `max_` field is `null` when unknown.

### Lifecycle

Every key is present, and `null` until the provider announces it. `status` becomes `deprecated` on `deprecated_on` and `retired` on `shutdown_on`. A model can also be `deprecated` while every date is still `null` ([retirements and model lines](lifecycle-and-lines.md)).

| Field | Meaning |
| --- | --- |
| `announced_on` | The day the provider announced the retirement, `YYYY-MM-DD` |
| `deprecated_on` | The day the model became deprecated: it still works, but the provider wants apps to move |
| `shutdown_on` | The day the provider stops serving the model |
| `replacement` | The id of the model of the same provider it recommends instead |
| `source` | The page where the provider announced it |

### Gateway

| Field | Meaning |
| --- | --- |
| `egress_dialect` | The API format a gateway sends the model's requests in, such as `openai_responses`; `null` for the provider's default ([APIs by provider](../concepts/provider-apis.md)) |
| `router` | `true` for a router that picks another model for each request |

## Prices

`pricing` holds decimal strings in US dollars per million tokens, and `null` where a price is unknown.

| Field | Meaning |
| --- | --- |
| `unit` | Always `usd_per_million_tokens` |
| `input` | Input tokens |
| `output` | Output tokens. With `output_image`, text output only |
| `cache_read` | Input read from the prompt cache |
| `cache_write` | Input written to the cache, with a five-minute lifetime |
| `cache_write_1h` | Input written to the cache, with a one-hour lifetime, where the provider documents one |
| `reasoning` | Reasoning tokens, where they cost apart from output |
| `input_audio`, `output_audio` | Audio input and output tokens |
| `input_image`, `output_image` | Image input and output tokens |
| `cache_read_image` | Image input read from the cache |
| `tiers` | Long-context prices, lowest `context_above` first, or empty ([long-context tiers](#long-context-tiers)) |
| `above_200k` | An older form of the long-context prices, kept for older readers and often `null`. Read `tiers`, which has the exact threshold |
| `per_unit` | Prices per image, second or other unit, for media models ([prices per unit](#prices-per-unit)) |

### Long-context tiers

A tier's prices apply to every token of a request whose prompt is longer than `context_above` tokens. A `null` price in a tier is the base price. For example, OpenAI's `gpt-5.4` costs more from 272,000 tokens on:

```json wrap
{ "context_above": 272000, "input": "5", "output": "22.5", "cache_read": "0.5", "cache_write": null }
```

| Field | Meaning |
| --- | --- |
| `context_above` | The prompt length in tokens above which the tier applies |
| `input`, `output`, `cache_read`, `cache_write` | The tier's prices |

### Prices per unit

Media models list `per_unit` prices: what an image, a second of video or a million characters costs. Each entry says what it applies to; a `null` condition applies to any value.

| Field | Meaning |
| --- | --- |
| `per` | `image`, `megapixel`, `video`, `second`, `minute`, `hour`, `million_characters` or `search` |
| `of` | Whether it counts what goes in, `input`, or comes out, `output` |
| `usd` | The price in US dollars, a decimal string |
| `size` | The image size it applies to, such as `1024x1024` |
| `quality` | The quality it applies to, such as `high` |
| `resolution` | The resolution it applies to, such as `1080p` |
| `duration` | For a price per video, the clip's length in seconds |
| `audio` | For video, whether the price is with sound |
| `minimum_seconds` | The shortest length billed for a time unit |
| `estimate` | `true` where the provider publishes only an estimate |

## How the gateway reads it

The gateway reads only the providers it calls and checks each model on its own:

- **A model it can't read is skipped**, and the rest of the catalogue still applies. That is a model whose key differs from its `id`, whose `pricing.unit` isn't `usd_per_million_tokens`, or whose price isn't a decimal from 0 to 1,000,000.
- **A tier or a line it can't read is left out**, never the model or the provider.
- **A catalogue in another format** is refused whole, and the data in use stays.
- **Values feed every project's catalogue** as KPMR's values, which a project can override ([models and aliases](../console/models-and-aliases.md#change-a-models-values)).

## Next steps

- [KPMR API](api.md): Fetch the catalogue and verify its signature.
- [Retirements and model lines](lifecycle-and-lines.md): See how the lifecycle dates and lines change over time.
- [Keeping model data current](sync.md): Choose how often the gateway fetches the catalogue.
