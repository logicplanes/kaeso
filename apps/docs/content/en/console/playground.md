---
title: Playground
description: Send one prompt to up to four models and compare their replies side by side.
order: 10
---

The **Playground** sends a prompt to up to four of the project's models at once and shows their replies side by side. Each column is its own conversation. Using it needs the right **Use the Playground**, which project admins and developers have.

It lists every kind of model the project can call, and each column takes the form of its model's kind: text, images, speech, sound effects, music, transcription or video. Embeddings, reranking and realtime models aren't listed.

## Compare models

1. Choose a model for the first column, and choose **Add column** for each further one. Search by name, id or provider, or narrow the list to one provider; models are grouped by provider, and the ones you chose recently come first. You can't choose a model whose provider has no key for this project, or one marked [Unavailable](models-and-aliases.md) because the provider doesn't serve it to the project's key. A model marked **Needs an API key** can't be chosen either: the project's key for its provider is an account, such as a [ChatGPT account](provider-keys.md#sign-in-with-a-chatgpt-account), which can't make images, speech or transcriptions.
2. Send a prompt to every model. When a chosen model reads files, the attach button adds them, named for what the models take, such as **Attach images**; dropping or pasting them into the prompt box works too. Each model gets the files it reads: images and PDFs for a text model, an image for an image model that edits, audio for a transcription model. A text model gets them again with every later turn.
3. Read the replies. A column follows a reply as it arrives until you scroll up, and follows again when you scroll back to its end. Each reply shows the time to the first token, the total time, the tokens and the cost. A reply that failed shows the reason in its column, and one the model cut off says why, such as **Stopped at the output limit**. **Retry** sends that turn again.

**Clear** empties every column's conversation; **Undo** in the notice brings it back.

The page's address holds the models and parameters, so a link opens the same comparison. A prompt you haven't sent yet stays in your browser. The conversation stays while the console is open in this tab, also when you visit other pages and come back. The console never stores it, so reloading or closing the tab ends it, and the browser asks first.

## Try media models

One prompt goes to every column, so a text model and an image model can answer it side by side. Each media column shows what its model made in place of a reply:

| Kind | You send | You get |
| --- | --- | --- |
| Images | A prompt, and an image to change when the model edits images | The images, each with **Download** |
| Speech | The text to speak | An audio player with **Download** |
| Sound effects, music | A description | An audio player with **Download** |
| Transcription | An audio file, attached or recorded with **Record audio**, and an optional note | The transcript |
| Video | A prompt | The job's progress with **Cancel**, then the video with **Download** |

A video job takes minutes. The Playground asks about it every few seconds, less often the longer it runs. **Cancel** or **Stop** cancels it at the provider. Only you see the videos you start in the Playground.

A media reply shows the time it took and its cost. The cost reads **Unknown** when the model has no price for what it made, such as a recording whose length the browser doesn't state.

## Set parameters

**Parameters** sets what every column sends:

| Parameter | What it does |
| --- | --- |
| System instructions | Instructions sent before the conversation |
| Temperature, Top P | Sampling, sent only to models that accept temperature |
| Max output tokens | The longest reply |
| Reasoning effort | How much a reasoning model thinks, from **None** to **Max** |
| Reasoning summary | Whether a reasoning model's thinking is summarized: **Auto**, **Concise**, **Detailed** or **Off** |

For media models, **Parameters** shows only what the chosen models take:

| Parameter | What it does |
| --- | --- |
| Image size, Images per prompt, Image quality | The size, how many images and the quality an image model makes |
| Voice | The voice a speech model speaks in |
| Length in seconds | How long a sound effect or a piece of music is |
| Transcript | **As spoken**, or **In English** for a transcription model that translates |

**Reset to defaults** clears every parameter except the system instructions.

## What a Playground request costs

Playground requests are real requests, charged to the project. They use the project's provider keys, your model access, and your limits and the project's. Key limits don't apply, because no key is used. The [request logs](request-logs.md) list them with the source **Playground**.

## Next steps

- [Models and aliases](models-and-aliases.md): Add the models you want to compare to the catalogue.
- [API keys](api-keys.md): Create a key to call the model you chose from an app.
