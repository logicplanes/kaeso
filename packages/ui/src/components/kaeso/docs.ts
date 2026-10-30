import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ClientButton: {
    description:
      "An agent client's mark, such as Claude, Cursor or GitHub Copilot, in a raised circle beside the other header buttons, as a button that opens how to connect it.",
    props: {
      "aria-label": "The client's name; also the tooltip.",
      client: "Which client's mark it shows.",
      onClick: "Opens how to connect the client.",
    },
  },
  ConversationView: {
    description:
      "A model conversation from a request log: role captions, prose, tool calls and results, reasoning, images, refusals. Inline images show; remote ones are links, so viewing a log never fetches them.",
    props: {
      density: "How much room each message gets.",
      footer: "Content after the last message, such as a composer.",
      markdown: "Draws text, reasoning and prose tool results as Markdown (safe: raw HTML stays text). The renderer loads the first time it is needed.",
      messages: "The conversation, message by message.",
    },
  },
  EffectiveAccess: {
    description:
      'What someone can do, at a glance: a row per group with the granted rights in words ("Every right", "None", or their names) and how many of the group\'s rights that is, under the roles it comes from; groups with nothing granted share one "No rights" row. A role chip with more to say has a dotted underline and says it in a tooltip. The detail, with a source per right, is RightsMatrix in read-only mode.',
    props: {
      action: "An action after the rows, such as a button that opens every right.",
      "aria-label": "The accessible name of the list.",
      groups: "The same groups as RightsMatrix; only `granted`, `label` and `code` are read.",
      sources: "What the access comes from, as chips above the rows: the member's roles.",
    },
  },
  GrantSummary: {
    description: "What an agent or a key may do, one short line each after a check mark, on a group surface with 20px inside, like a settings group.",
    props: {
      action: "A small button after the note (or alone, at the end), such as Customize.",
      "aria-label": 'The list\'s accessible name ("What it can do").',
      footer: "A note under the lines in secondary text, such as how long it lasts.",
      items: "The lines, short and without a final period.",
    },
  },
  EnvironmentSnippet: {
    description: "The gateway's base URL and a key placeholder to copy, with snippets per SDK.",
    props: {
      apiKey: "What stands in for the key: the real key right after creation, a placeholder elsewhere.",
      baseUrl: "The gateway's address.",
      model: "A model id for the snippet.",
    },
  },
  KeyDisplay: {
    description: "A redacted key: mono 13, a word badge for any state but active, a copy of the redacted form, and its access in an outline badge.",
    props: {
      access: '"Inference", "Platform", or both.',
      copy: "Hide the copy circle.",
      redacted: "The redacted key as the API returns it: sk-kso-AbC123…wxyz",
      state: "The key's state; disabled, expired and archived keys are shown dimmed.",
      stateBadge: "Shows the state's badge beside a key that isn't active (on by default); turn it off where a heading already names the state.",
    },
  },
  LimitMeter: {
    description:
      "A limit: its kind, scope and window, hard or soft, the meter with alert ticks, and when it resets. Over the limit it says so, and what happens to requests.",
    props: {
      alerts: "Alert thresholds as ratios.",
      hard: "Whether requests are refused at the limit (`true`) or only alerted (`false`).",
      kind: "What is limited: requests, tokens or spend, and over which period.",
      limit: "The limit.",
      resets: '"Resets in 12 days".',
      scope: '"Project", "Key ci-runner", "Member Ada". A text scope also names the meter for screen readers, and shows whole on hover when it doesn\'t fit.',
      used: "How much is used.",
      valueLabel: 'Formatted values: "$32.10 of $50.00".',
      window: '"Per minute", "Per month".',
    },
  },
  ModelLabel: {
    description:
      "A model: provider mark, display name, then its id in mono, capability icons with tooltips. Recognition over recall. Never wider than its container: the id gives way first, ending in an ellipsis, and goes when fewer than a few of its characters would show (a short id shows whole or not at all); the name ends in an ellipsis last. On a phone the capability icons give way (the model's sheet lists them), where they were clipped at the row's edge.",
    props: {
      capabilities: "What the model can do, shown as small marks after its name.",
      mark: "Shows the provider's mark first (default); turn it off in a list grouped under each provider's mark.",
      stacked: "With a name, puts the model id on its own line under it, so neither is cut short; for rows tall enough for two lines.",
      model: "The model id, in mono: after the name, quieter, or alone when there's no name.",
      name: "The display name, first.",
      provider: "The provider id, such as `openai` or `anthropic`.",
      providerName: "The provider's name from the gateway's provider list, which names its mark for screen readers; the id without it.",
    },
  },
  ProviderBadge: {
    description:
      "A provider's mark on a quiet tile, in the provider's own one-colour version for the theme; for providers without one, the first letter of their name.",
    props: {
      name: "The provider's name from the gateway's provider list: the badge's accessible name, the text `withName` writes and the source of the letter; the id without it.",
      provider: "The provider id, such as `openai` or `anthropic`.",
      size: "The badge's size.",
      withName: "Writes the provider's name next to its mark.",
    },
  },
  CapabilityIcons: {
    description: "A model's capabilities as small icons, each named in a tooltip and read as one list by screen readers.",
    props: {
      capabilities: "The capabilities: tools, reasoning, vision, pdf, structuredOutput.",
    },
  },
  ProviderCard: {
    description: "A provider and how this place reaches it: its mark, name and connection, the key in use, and the actions to connect, replace or remove one.",
    props: {
      actions: "Buttons: one primary at most.",
      connected: "Whether requests can reach the provider.",
      detail: "A line under the provider's name, such as how many keys it has.",
      keyLabel: "The key: its name and last four characters, and when it was last used.",
      name: "The provider's name from the gateway's provider list, as the card's title; the id without it.",
      provider: "The provider id, such as `openai` or `anthropic`.",
      status: 'Where the key comes from, or what\'s missing: "The organization\'s key", "No key yet".',
    },
  },
  ProviderPicker: {
    description:
      "Choosing a provider from any number of them: a search field over every provider, connected ones first with their key counts, then the rest from A to Z, each with its mark. Always open in a dialog step, or a pill that opens it in forms and toolbars.",
    props: {
      description: "One sentence of help under the label.",
      label: "The visible label.",
      layout: "The label above the control (`vertical`) or beside it (`horizontal`).",
      optional: "Adds “Optional” to the label, for forms where most fields are required.",
      required: "Marks it as required in a form.",
      allLabel: 'Select only: a first option for no provider in particular, such as "All providers"; choosing it reports `null`.',
      appearance: "An always-open list for a dialog step, tiles with each provider's mark for a dialog's first step, or a pill that opens the list.",
      "aria-label":
        "What the picker is for, when there is no visible label: it names the list, and the pill, named by the provider it shows, reads it as its description.",
      autoFocus: "List only: focuses the search field when it appears.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      onValueChange: "Called with the chosen provider's id, or `null` for `allLabel`.",
      onChoose: "With `grid`: called when a tile is pressed or clicked, not when the arrow keys move the choice, so a dialog step can continue at once.",
      placeholder: "The search field's placeholder; in a pill without `allLabel`, also the text shown while nothing is chosen.",
      providers: "The providers: id, name, whether the organization has connected it, and a short hint such as its key count.",
      size: "Select only: the height of the pill.",
      value: "The chosen provider's id, when you control it.",
    },
    deprecated: {
      error:
        "Errors are toasts, never text under a field: `error` marks the control invalid and stays its description for screen readers, but isn't drawn. Pass `invalid` and toast the message.",
    },
  },
  RequestTimeline: {
    description:
      "A bar of time to first byte against the total: 240px (default), 160px in a cell, or the width of its container (`full`) in an inspector. Under it each time sits over its label, so the line of times never wraps and a label that runs long in a language wraps under its own time: never cut, and the same height whatever the times are.",
    props: {
      totalMs: "The total time, in milliseconds.",
      ttfbMs: "The time to the first byte, in milliseconds.",
      width: "The width of the bar in pixels, or the full width.",
    },
  },
  RightsMatrix: {
    description:
      'Rights by group, for a role, a member or a key. Edit mode: a checkbox per right and per group (checked only when every right in it is granted, mixed when some are), requirements kept (granting "edit" grants "view"; withdrawing "view" withdraws "edit"), each right\'s needs said under it, and a right that can\'t change saying why under it. The matrix is one tab stop: the arrow keys move between checkboxes, and the right arrow reaches a right\'s sensitive mark. A group\'s name toggles the group, as a right\'s row toggles the right. Read-only mode: a mark per right and the roles that grant it. Rows keep their size whatever their state. Fits a sheet and a phone.',
    props: {
      "aria-label": "The accessible name of the matrix.",
      groups: "The rights, by category, with who holds each.",
      onChange: "The right the person toggled. With requirements, prefer `onRightsChange`, which has every change at once.",
      onRightsChange: "Every change one toggle makes, the toggled right first, then what it required or what required it.",
      originLabels: 'What the origin column says: `{ preset: "From Developer" }`. Defaults to "From role", "Added" and "Removed" in the provider\'s language.',
      readOnly: "No checkboxes: a mark per right and the roles that grant it.",
      searchable: "A search field above the groups that filters rights by name, code, description or group.",
    },
  },
  RolePicker: {
    description:
      "Choosing a role: built-in roles first, then custom ones under their own caption, each with its one-sentence description. A role the person can't give stays in the list, disabled, with the reason under it, so nobody wonders where it went.",
    props: {
      appearance: "A list with each role's description, or a compact select.",
      "aria-label": "The accessible name, for when there is no visible label.",
      disabled: "Turns it off: it can't be used, and it looks dimmed.",
      fullWidth: "Select only: fill the container.",
      label: "The select's visible label; a list is named by `aria-label` or the step around it.",
      name: "The field name the value is sent under in a form.",
      onValueChange: "Called with the chosen role's id.",
      roles: "The roles to choose from.",
      size: "The height of the control.",
      value: "The chosen role's id, when you control it.",
    },
  },
  SecretReveal: {
    description:
      'The one-time secret: calm, centered, certain. The secret on a well (selected on click), a plain warning, a large "Copy key" that becomes "Key copied" without changing size, and a `.env` tab with the gateway\'s URL and the key beside SDK snippets that read the key from the environment.',
    props: {
      apiKey: "What stands in for the key: the real key right after creation, a placeholder elsewhere.",
      baseUrl: "The gateway's address, for the `.env` tab and the code snippets.",
      model: "A model id for the code snippet.",
      name: "The key's name, for the heading.",
      onCopied: "Called when the secret was copied.",
      secret: "The full secret, shown once.",
      snippets: "What follows the secret: the SDK snippets for calling models (default), other snippets (a key for the Platform API), or none (`false`).",
    },
  },
  StatusCode: {
    description: "An HTTP status: the code in mono, in its class color (with its phrase in a tooltip).",
    props: {
      code: "The HTTP status code.",
      showText: "Writes the status's name next to its code.",
    },
  },
  UsageBreakdown: {
    description:
      "A stacked bar of token kinds with the numbers under it, in the fixed series mapping; cache read is a soft blue edged in input's blue. Each rounded count shows the exact one on hover and focus.",
    props: {
      tokens:
        "Tokens by kind: input, output, cache reads and writes, reasoning. A kind left out isn't listed, for kinds that can't apply, such as output for embeddings; pass 0 to list it.",
    },
  },
} satisfies ComponentDocs;
