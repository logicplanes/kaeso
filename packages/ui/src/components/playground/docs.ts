import type { ComponentDocs } from "../../component-docs";

export const docs = {
  CompareGrid: {
    description: "Columns side by side to compare models: equal widths, a header, a body that scrolls on its own and a footer each.",
    props: {
      active: "The column on screen on a phone, or scrolled to when the columns don't fit.",
      addLabel: "The add circle's name. Defaults to \"Add model\" in the provider's language.",
      "aria-label": 'The grid\'s accessible name ("Models").',
      children: "`CompareGrid.Column` elements, one to four.",
      divider: "A hairline between columns, or none.",
      max: "Columns at most (4 by default): the add circle is disabled there, with the reason in its tooltip.",
      onActiveChange: "Called with the index of the column in focus.",
      onAdd: "Adds a column: a circle at the end of the last column's header (and in the phone's switcher).",
    },
  },
  Composer: {
    description:
      "The prompt box: a text box that grows with what is written up to `maxRows`, then scrolls; Send, which becomes Stop while a response streams. Enter sends and Shift Enter adds a line (`submitOn`); ⌘ Enter or Ctrl Enter also sends. On a touch screen Enter adds a line and the button sends.",
    props: {
      actions: "Controls at the start of the button row: parameters, clear.",
      "aria-label": 'The text box\'s accessible name ("Prompt" by default).',
      autoFocus: "Focuses the composer when it appears.",
      clearOnSubmit: "Clears an uncontrolled composer after Send (true by default).",
      defaultValue: "The message on first render.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      loading: "The prompt is being sent: Send shows a spinner.",
      maxRows: "Lines before the text box scrolls (10 by default).",
      name: "The field name the value is sent under in a form.",
      onStop: "Stop: the button while `streaming`.",
      onSubmit:
        "Send: the Send button, Enter in the text box (see `submitOn`), or ⌘ Enter (Ctrl Enter). Not called for a prompt that is empty or only spaces, while disabled, sending or streaming, or while an input method is composing.",
      onValueChange: "Called with the message as it is typed.",
      placeholder: "Text shown while there is no value.",
      size: "The composer's height.",
      stopLabel: "The stop button's accessible name, shown while a reply streams.",
      streaming: "A response is arriving: Send becomes Stop. The text box stays editable for the next prompt.",
      submitLabel: "The send button's accessible name.",
      submitOn:
        'The key that sends from the text box. `"enter"` (default): Enter sends and Shift Enter adds a line; on a touch screen Enter adds a line and the Send button sends. `"mod-enter"`: Enter adds a line and only ⌘ Enter (Ctrl Enter) sends. ⌘ Enter and Ctrl Enter send with either.',
      value: "The message being written, when you control it.",
    },
  },
  ModelPicker: {
    description: "A model, picked from the project's catalogue: the provider mark, name and id (ModelLabel), grouped by provider.",
    props: {
      appearance: "A quiet button (`ghost`) or a filled one (`default`).",
      "aria-label": 'The picker\'s accessible name ("Model" by default).',
      defaultValue: "The chosen model on first render.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      groupByProvider: "Groups the list by provider (true by default when there are several).",
      onValueChange: "Called with the chosen model.",
      options: "The models to choose from.",
      placeholder: "Text shown while there is no value.",
      size: "The button's height.",
      value: "The chosen model, when you control it.",
    },
  },
  ParameterPanel: {
    description:
      "A request's parameters: system instructions, temperature, top P, most output tokens, and, for a model that reasons, reasoning effort and summary. Unset values are the model's defaults; Reset returns to them (instructions stay).",
    props: {
      defaults: "What unset shows (temperature 1 and top P 1 unless given): the model's defaults.",
      disabled: "Turns off every parameter.",
      instructions: "Shows the system instructions field (true by default).",
      layout: "Compact inside a popover, or roomier in a side panel.",
      maxOutputTokensLimit: "The model's most output tokens: the field's maximum.",
      onValueChange: "Called with the changed parameters.",
      reasoning: "The reasoning efforts the model takes, or false (the default) when it doesn't reason: no effort and no summary.",
      sampling: "Shows temperature and top P; models that reason often take neither (true by default).",
      value: "The parameters, such as temperature and effort.",
    },
  },
  ParameterPopover: {
    description: "ParameterPanel in a 320px popover from a sliders circle (or any trigger).",
    props: {
      align: "How it lines up with what it belongs to.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      defaults: "What unset shows (temperature 1 and top P 1 unless given): the model's defaults.",
      disabled: "Turns off the button that opens the parameters.",
      instructions: "Shows the system instructions field (true by default).",
      maxOutputTokensLimit: "The model's most output tokens: the field's maximum.",
      onOpenChange: "Called when it opens or closes.",
      onValueChange: "Called with the changed parameters.",
      open: "Whether it is open, when you control it.",
      reasoning: "The reasoning efforts the model takes, or false (the default) when it doesn't reason: no effort and no summary.",
      sampling: "Shows temperature and top P; models that reason often take neither (true by default).",
      side: "The side it opens on.",
      title: "The popover's title, also its name. Defaults to \"Parameters\" in the provider's language.",
      trigger: 'The button that opens it: a sliders circle named "Parameters" by default.',
      value: "The parameters, such as temperature and effort.",
    },
  },
  PromptSuggestions: {
    description:
      "Example prompts before a thread starts: up to three lines of plain secondary text that take a quiet fill on hover. No outlines, no icons; leave them out once a prompt is sent.",
    props: {
      align: "Aligns the suggestions at the start or in the center.",
      "aria-label": "The list's accessible name. Defaults to \"Example prompts\" in the provider's language.",
      onSelect: "Puts the prompt in the composer (or sends it).",
      suggestions: "Up to three short prompts; more are left out.",
    },
  },
  ResponseMetrics: {
    description:
      'A response\'s numbers on one line: time to first token, total time, tokens in → out (and reasoning), cost. Tabular figures, labels in tertiary text. While `live`, values not known yet show "—" in the room they will take; before any response the line is empty, its height kept.',
    props: {
      cost: 'USD as a decimal string: "0.004213".',
      duration: "Milliseconds from sending to the last token.",
      inputTokens: "The tokens sent, once known.",
      live: 'A response is arriving: values not known yet show a quiet "—" in the room they will take.',
      outputTokens: "The tokens received, once known.",
      reasoning: "The model reasons: while `live`, the reasoning item waits with a placeholder.",
      reasoningTokens: "Of the output tokens, those spent reasoning. Shown when given, or when `reasoning` is set.",
      size: "The text size.",
      startedAt: "When the request was sent (Date.now()): while live, Total counts up quietly until `duration` arrives.",
      timeToFirstToken: "Milliseconds from sending to the first token.",
    },
  },
  ResponseView: {
    description:
      'A model\'s response as it arrives: Markdown text for reading (15/22), the reasoning summary in a "Thinking" disclosure (closed by default, live while it streams), and calm states for a stop or a failure. Errors themselves are toasts; the response says only that there is none.',
    props: {
      "aria-label": 'The response\'s accessible name ("GPT-5 response" in a comparison). Defaults to "Response" in the provider\'s language.',
      defaultReasoningOpen: "Whether the reasoning starts expanded.",
      footer: "Under the response: its metrics, actions.",
      onReasoningOpenChange: "Called when the reasoning is expanded or collapsed.",
      onRetry: "Sends the turn again; shown with an error or a stop.",
      reasoning: 'The reasoning summary so far, as Markdown. A "Thinking" disclosure shows it, closed by default.',
      reasoningDuration: 'How long the model reasoned, in milliseconds: "Thought for 4.2 s".',
      reasoningOpen: "Whether the reasoning is expanded, when you control it.",
      status: "Where the reply stands: waiting, streaming, done, stopped or failed.",
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
    description: 'A person\'s turn: their prompt as quiet text that opens the turn, no bubble. A long prompt shows its first lines until "Show all".',
    props: {
      "aria-label": "The message's accessible name. Defaults to \"Prompt\" in the provider's language.",
      children: "The prompt as it was written: plain text, line breaks kept.",
      maxLines: 'Lines shown before "Show all" (4 by default; 0 shows everything).',
    },
  },
} satisfies ComponentDocs;
