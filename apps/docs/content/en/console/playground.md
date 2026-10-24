---
title: Playground
description: Send one prompt to up to four models and compare their replies side by side.
order: 9
---

The **Playground** sends a prompt to up to four of the project's models at once and shows their replies side by side. Each column is its own conversation. Using it needs the right **Use the Playground**, which project admins and developers have.

## Compare models

1. Choose a model for the first column, and choose **Add column** for each further one. Search by name, id or provider, or narrow the list to one provider; models are grouped by provider, and the ones you chose recently come first. You can't choose a model whose provider has no key for this project, or one marked [Unavailable](models-and-aliases.md) because the provider doesn't serve it to the project's key.
2. Send a prompt to every model.
3. Read the replies. Each shows the time to the first token, the total time, the tokens and the cost. A reply that failed shows the reason in its column, and one the model cut off says why, such as **Stopped at the output limit**. **Retry** sends that turn again.

**Clear** empties every column's conversation; **Undo** in the notice brings it back.

The page's address holds the models and parameters, so a link opens the same comparison. A prompt you haven't sent yet stays in your browser. The conversation stays while the console is open in this tab, also when you visit other pages and come back. The console never stores it, so reloading or closing the tab ends it, and the browser asks first.

## Set parameters

**Parameters** sets what every column sends:

| Parameter | What it does |
| --- | --- |
| System instructions | Instructions sent before the conversation |
| Temperature, Top P | Sampling, sent only to models that accept temperature |
| Max output tokens | The longest reply |
| Reasoning effort | How much a reasoning model thinks, from **None** to **Max** |
| Reasoning summary | Whether a reasoning model's thinking is summarized: **Auto**, **Concise**, **Detailed** or **Off** |

**Reset to defaults** clears every parameter except the system instructions.

## What a Playground request costs

Playground requests are real requests, charged to the project. They use the project's provider keys, your model access, and your limits and the project's. Key limits don't apply, because no key is used. The [request logs](request-logs.md) list them with the source **Playground**.

## Next steps

- [Models and aliases](models-and-aliases.md): Add the models you want to compare to the catalogue.
- [API keys](api-keys.md): Create a key to call the model you chose from an app.
