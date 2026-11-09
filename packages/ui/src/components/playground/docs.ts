import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Attachments: {
    description:
      "The files sent with a prompt, in a row that wraps: an image as a 64px thumbnail, a PDF, audio or any other file as a pill with its name. In the composer each has a remove button; above a sent prompt they line up at the end, over its bubble.",
    props: {
      align: "Where the row lines up: the start (in the composer) or the end (above a sent prompt).",
      "aria-label": 'The list\'s accessible name ("Attachments" by default).',
      items: "The files: an id, the name, the media type and, for an image, a `blob:` or `data:` URL to draw it from.",
      onRemove: "Removes a file by its id: a remove button on each thumbnail and pill, named after the file.",
    },
  },
  CompareGrid: {
    description:
      "Columns side by side to compare models: equal widths of at least 320px, a header, a body that scrolls on its own and a footer each. When the columns don't fit, Previous column and Next column reach the hidden ones. A column's body follows its end as content arrives while the reader is there, lets go the moment they scroll up, and follows again when they return to the end or its `followKey` changes. Removing a column is instant, so offer Undo in a toast.",
    props: {
      active: "The column on screen on a phone, or scrolled to when the columns don't fit.",
      addLabel: "The add circle's name. Defaults to \"Add model\" in the provider's language.",
      "aria-label": 'The grid\'s accessible name ("Models").',
      children: "`CompareGrid.Column` elements, one to four.",
      divider: "A hairline between columns, or none.",
      max: "Columns at most (4 by default): the add circle is disabled there, with the reason in its tooltip.",
      onActiveChange: "Called with the index of the column in focus.",
      onAdd:
        "Adds a column: a circle at the end of the last column's header (on a phone with several columns, in the switcher). Focus moves to the new column's header.",
    },
  },
  Composer: {
    description:
      "The prompt box: a text box that grows with what is written up to `maxRows`, then scrolls, its edge fading where more lines wait; under it a row with the caller's actions at the start and a round Send arrow at the end, which becomes a Stop square while a response streams. Enter sends and Shift Enter adds a line (`submitOn`); ⌘ Enter or Ctrl Enter also sends. On a touch screen Enter adds a line and the button sends. A click anywhere in the box, the button row included, puts the cursor in the text.",
    props: {
      accept: "The file types the Attach button offers, as an `accept` attribute (`image/*,application/pdf`).",
      actions: "Controls at the start of the button row: parameters, clear. Ghost and secondary buttons there take tones made for the composer's surface.",
      attachLabel: 'The Attach button\'s name and tooltip ("Attach files" by default).',
      attachments: "Files waiting to go with the prompt, drawn above the text box; with any, Send works without text.",
      "aria-label": 'The text box\'s accessible name ("Prompt" by default).',
      autoFocus: "Focuses the composer when it appears.",
      clearOnSubmit: "Clears an uncontrolled composer after Send (true by default).",
      defaultValue: "The message on first render.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      loading: "The prompt is being sent: Send shows a spinner.",
      maxRows: "Lines before the text box scrolls (10 by default).",
      name: "The field name the value is sent under in a form.",
      onStop: "Stop: the button while `streaming`.",
      onAttach:
        "Takes files: an Attach button at the start of the button row, files dropped anywhere on the composer (its outline darkens while they hover) and files pasted into the text box. Check their type and size here and say what was refused.",
      onRemoveAttachment: "Removes a waiting file by its id.",
      onSubmit:
        "Send: the Send button, Enter in the text box (see `submitOn`), or ⌘ Enter (Ctrl Enter). Not called for a prompt that is empty or only spaces, while disabled, sending or streaming, or while an input method is composing.",
      onValueChange: "Called with the message as it is typed.",
      placeholder: "Text shown while there is no value.",
      size: "The composer's height.",
      stopLabel: 'The Stop button\'s name and tooltip, while a response streams ("Stop" by default).',
      streaming:
        "A response is arriving: Send becomes Stop. The text box stays editable for the next prompt; Enter waits, and the text box's description says to stop the response to send.",
      submitLabel: 'The Send button\'s name and tooltip ("Send" by default).',
      submitOn:
        'The key that sends from the text box. `"enter"` (default): Enter sends and Shift Enter adds a line; on a touch screen Enter adds a line and the Send button sends. `"mod-enter"`: Enter adds a line and only ⌘ Enter (Ctrl Enter) sends. ⌘ Enter and Ctrl Enter send with either.',
      value: "The message being written, when you control it.",
    },
  },
  MediaResponse: {
    description:
      "What an image, speech, sound or video model made, in the place a text response takes: images in a grid of squares, audio in a player a row tall, a video in a 16:9 player, each with Download. While it is made, quiet placeholders of the same size hold the room; a video job shows its progress, the seconds it has taken and Cancel. A stop or a failure reads as in ResponseView.",
    props: {
      "aria-label": 'The response\'s accessible name ("GPT Image 2 response" in a comparison). Defaults to "Response" in the provider\'s language.',
      error: 'Why it failed, shown under "No response" with status "error".',
      expected: "How many images to hold room for while they are made.",
      footer: "Under the response, after Retry: its metrics.",
      items:
        "The files: where to show or play each from (`blob:`, `data:` or a same-origin link), the file name it downloads under, and a label in the reader's language that Download and screen readers say.",
      kind: "What the model makes: images, audio or a video.",
      onCancel: "Cancels a running video job: a Cancel button under its progress.",
      onRetry: "Sends the turn again: a Retry button after a stop or a failure, and a quiet Retry circle under a finished response.",
      progress: "A video job's progress from 0 to 100, when its provider says; the bar pulses quietly while it doesn't.",
      status: "Where it stands: waiting, running, done, stopped or failed.",
    },
  },
  ModelPicker: {
    description:
      "A model from a catalogue of any size: a pill showing the chosen model that opens a searchable list, grouped by provider under sticky captions with marks and counts, connected providers first and recent models on top, with a Provider filter that lists only the providers whose models match the search. Each row shows the name, the id, capabilities, context and price; deprecated models wait behind a toggle.",
    props: {
      description: "One sentence of help under the label.",
      label: "The visible label.",
      layout: "The label above the control (`vertical`) or beside it (`horizontal`).",
      optional: "Adds “Optional” to the label, for forms where most fields are required.",
      required: "Marks it as required in a form.",
      allLabel: 'Single only: a first option for no model in particular, such as "All models"; choosing it reports an empty string.',
      appearance: "A quiet button (`ghost`) or a filled one (`default`).",
      "aria-label":
        'What the picker is for ("Model" by default): it names the list and its popover, and the button, named by the model it shows, reads it as its description.',
      defaultValue: "The chosen model, or models with `multiple`, on first render.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      fullWidth: "Fills the container.",
      groupByProvider: "Groups the list by provider (true by default when there are several).",
      invalid: "Shows it as invalid; pair it with a message that says what to fix.",
      multiple: "Allows choosing several models; `value` is then a list.",
      onOpenChange: "Called when the list opens or closes.",
      onValueChange: "Called with the chosen model, or the list of models with `multiple`.",
      open: "Whether the list is open, when you control it.",
      options:
        "The models: value, provider, id, name, capabilities, context window, a formatted price, a short note such as why it is disabled, and whether it is deprecated.",
      placeholder: "Text shown while there is no value.",
      providers: "The providers' names and whether each is connected; connected providers come first.",
      recent: "Values of recently used models, shown first while the search is empty.",
      size: "The button's height.",
      value: "The chosen model, or models with `multiple`, when you control it.",
    },
    deprecated: {
      error:
        "Errors are toasts, never text under a field: `error` marks the control invalid and stays its description for screen readers, but isn't drawn. Pass `invalid` and toast the message.",
    },
  },
  ParameterPanel: {
    description:
      "A request's parameters: system instructions, temperature, top P, maximum output tokens, and, for a model that reasons, reasoning effort and summary; for media models, the image size, count and quality, the voice, the length in seconds and whether a transcript is in English. Unset values are the model's defaults; Reset returns to them (instructions stay). With a `trigger`, it opens in a 320px popover with a Close button beside its title, and focus stays inside until it closes.",
    props: {
      align: "How the popover lines up with its trigger.",
      defaultOpen: "Whether the popover starts open, when the component keeps its own state.",
      defaults: "What unset shows (temperature 1 and top P 1 unless given): the model's defaults.",
      disabled: "Turns off every parameter.",
      duration: "Shows the length in seconds, for sound effect and music models.",
      image: "An image model's options: the size, how many images per prompt (up to four shown) and its qualities.",
      instructions: "Shows the system instructions field (true by default).",
      layout: "Compact inside a popover, or roomier in a side panel.",
      maxOutputTokensLimit: "The model's maximum output tokens: the field's maximum, named in a hint under it.",
      onOpenChange: "Called when the popover opens or closes.",
      onValueChange: "Called with the changed parameters.",
      open: "Whether the popover is open, when you control it.",
      reasoning: "The reasoning efforts the model takes, or false (the default) when it doesn't reason: no effort and no summary.",
      sampling: "Shows temperature and top P; models that reason often take neither (true by default).",
      side: "The side the popover opens on.",
      text: "Shows the text parameters, such as the maximum output tokens (true by default); off when no text model is chosen.",
      title: "The popover's title, also its name. Defaults to \"Parameters\" in the provider's language.",
      translate: "Shows whether a transcript is as spoken or in English, for transcription models that translate.",
      trigger: "The button that opens the parameters in a popover; without one, the parameters sit in the page.",
      value: "The parameters, such as temperature and effort, image size and voice.",
      voices: "A speech model's voices to choose from, or true for a field that takes any voice name or id.",
    },
  },
  Reasoning: {
    description:
      "A model's reasoning behind a quiet disclosure, closed by default: a chevron, then “Thought for 4.2 s” when it is done, or “Thinking” with the latest line beside it while it streams. Opened, the reasoning reads in small secondary text under the title, beside a line under the chevron. The Playground and the request log (ConversationView) show reasoning with it.",
    props: {
      children: "What the open disclosure shows in place of `text`, such as why the reasoning can't be shown.",
      defaultOpen: "Whether it starts open.",
      duration: "How long the model reasoned, in milliseconds: “Thought for 4.2 s”.",
      hint: "A short note beside the title, such as “Redacted by the provider”; while `live`, the latest line of the reasoning shows there instead.",
      live: "The reasoning is still arriving: “Thinking” pulses quietly and its latest line shows beside it.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      text: "The reasoning, as Markdown.",
      title: "The title in place of “Thinking” or “Thought for …”, such as “Reasoning” in a request log.",
    },
  },
  ResponseMetrics: {
    description:
      'A response\'s numbers on one line: time to first token, total time, tokens ("412 in · 168 out", and reasoning), cost with the decimals it was given, as request logs show it. Tabular figures, labels in tertiary text. Every value keeps the room a typical value takes, so nothing moves when values arrive; while `live`, values not known yet show "—"; before any response the line is empty, its height kept.',
    props: {
      costKnown: 'False when the model has no price for what it made: the cost reads "Unknown" (true by default).',
      cost: 'USD as a decimal string, shown with its own decimals (two at least, six at most): "0.004213" reads "$0.004213", "1.2311" reads "$1.2311", zero reads "$0" and less than a millionth reads "< $0.000001".',
      duration: "Milliseconds from sending to the last token.",
      inputTokens: "The tokens sent, once known.",
      live: 'A response is arriving: values not known yet show a quiet "—" in the room they will take.',
      outputTokens: "The tokens received, once known.",
      reasoning: "The model reasons: while `live`, the reasoning item waits with a placeholder.",
      reasoningTokens: 'Of the output tokens, those spent reasoning. Shown when given, or when `reasoning` is set ("—" when the provider didn\'t report them).',
      size: "The text size.",
      startedAt:
        "When the request was sent (Date.now()): while live, Total counts up quietly, in tenths of a second and in whole seconds past ten, until `duration` arrives.",
      timeToFirstToken: "Milliseconds from sending to the first token.",
    },
  },
  ResponseView: {
    description:
      'A model\'s response as it arrives: nothing until the first token, then Markdown text for reading (15/22) that grows as it streams; the reasoning summary in a "Thinking" disclosure (closed by default, live while it streams); and calm states for a stop, a cut-off reply or a failure, which shows its error in place. Text sits 12px in, code and tables on the edge; screen readers hear when a response finishes or stops.',
    props: {
      "aria-label": 'The response\'s accessible name ("GPT-5 response" in a comparison). Defaults to "Response" in the provider\'s language.',
      defaultReasoningOpen: "Whether the reasoning starts expanded.",
      error:
        'Why the reply failed, shown under "No response" with status "error": the gateway\'s message in the reader\'s language; `code` spans render as code.',
      footer: "Under the response, after Copy and Retry on their row: its metrics.",
      incompleteReason:
        'Why the model cut the reply off, with status "incomplete": `max_output_tokens` reads "Stopped at the output limit", `content_filter` names the filter.',
      onReasoningOpenChange: "Called when the reasoning is expanded or collapsed.",
      onRetry: "Sends the turn again: a Retry button after a stop, a cut-off reply or an error, and a quiet Retry circle under a finished reply.",
      reasoning: 'The reasoning summary so far, as Markdown. A "Thinking" disclosure shows it, closed by default.',
      reasoningDuration: 'How long the model reasoned, in milliseconds: "Thought for 4.2 s".',
      reasoningOpen: "Whether the reasoning is expanded, when you control it.",
      status: "Where the reply stands: waiting, streaming, done, stopped, cut off by the model (incomplete) or failed.",
      text: "The output text so far, as Markdown.",
    },
  },
  Thread: {
    description: "A conversation's turns, 32px apart: spacing, not lines or boxes, separates them.",
    props: {
      children: "`UserMessage` and `ResponseView` turns, oldest first.",
    },
  },
  UserMessage: {
    description:
      'A person\'s turn: their prompt in a bubble on a quiet fill at the end of the line, so it reads apart from the response that starts at the other edge. A long prompt shows its first lines until "Show all", a disclosure button under them.',
    props: {
      "aria-label": "The message's accessible name. Defaults to \"Prompt\" in the provider's language.",
      attachments: "The files sent with the prompt, above its bubble: images as thumbnails, other files by name.",
      children: "The prompt as it was written: plain text, line breaks kept.",
      maxLines: 'Lines shown before "Show all" (4 by default; 0 shows everything).',
    },
  },
} satisfies ComponentDocs;
