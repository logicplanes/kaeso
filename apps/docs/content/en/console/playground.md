---
title: Playground
description: Send a prompt to up to four models side by side.
order: 8
---

The **Playground** sends a prompt to up to four of the project's models at once and shows their replies side by side. Each column is its own conversation. Using it needs the right **Use the Playground**, which project admins and developers have.

Playground requests are real requests. They use the project's provider keys, your model access, and your and the project's limits; they are charged to the project and appear in the [request logs](request-logs.md) with the source **Playground**. Key limits don't apply, because no key is used.

## Compare models

1. Choose a model for the first column, and **Add column** for each further one. A model whose provider has no key for this project can't be chosen.
2. Send a prompt to every model.
3. Each reply shows the time to the first token, the total time, the tokens and the cost.

**Clear** empties every column's conversation.

The models and parameters are part of the page's address, so a link opens the same comparison. A prompt you haven't sent yet stays in your browser; conversations are not saved.

## Parameters

**Parameters** sets what every column sends:

| Parameter | What it does |
| --- | --- |
| System instructions | Instructions sent before the conversation |
| Temperature, Top P | Sampling, sent only to models that accept temperature |
| Max output tokens | The longest reply |
| Reasoning effort | How much a reasoning model thinks, from **None** to **Max** |
| Reasoning summary | Whether a reasoning model's thinking is summarized: **Auto**, **Concise**, **Detailed** or **Off** |

**Reset to defaults** clears every parameter except the system instructions.
