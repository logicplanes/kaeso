---
title: Playground
description: Send a prompt to up to four models side by side.
order: 8
---

The **Playground** sends a prompt to one or more models of the project and shows their replies side by side, up to four at once. Each column is its own conversation.

Playground requests are real requests. They use the project's provider keys, your model access, and your and the project's limits and budgets. They are charged to the project and appear in the request logs with the source **Playground**. Key limits don't apply, as no key is used.

## Use it

1. Choose a model for the first column; **Add column** adds more.
2. Type a prompt and send it.
3. Each reply shows the time to the first token, the total time, the tokens and the cost.

**Clear** empties every column's conversation.

A model whose provider has no key for this project can't be chosen.

## Parameters

**Parameters** sets what is sent with every column:

| Parameter | What it does |
| --- | --- |
| System instructions | Instructions sent before the conversation |
| Temperature, Top P | Sampling, sent only to models that take temperature |
| Max output tokens | The longest reply |
| Reasoning effort | How hard a reasoning model thinks, from none to max |
| Reasoning summary | Whether a reasoning model's thinking is summarized: auto, concise, detailed or off |

**Reset to defaults** clears them.

The models and parameters are part of the page's address, so a link to it opens the same comparison. The prompt you haven't sent yet is kept in your browser; conversations are not saved.

Using the Playground needs the right **Use the Playground**, which project admins and developers have.
