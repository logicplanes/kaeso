import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ConversationView: {
    description: "A model conversation from a request log: role captions, prose, tool calls and results, reasoning, images, refusals.",
    props: {
      density: "How much room each message gets.",
      footer: "Content after the last message, such as a composer.",
      markdown: "Draws text, reasoning and prose tool results as Markdown (safe: raw HTML stays text). The renderer loads the first time it is needed.",
      messages: "The conversation, message by message.",
    },
  },
  EffectiveAccess: {
    description:
      'What someone can do, at a glance: a row per group with the granted rights in words ("Every right", "None", or their names) and how many of the group\'s rights that is, under the roles it comes from. The detail, with a source per right, is RightsMatrix in read-only mode.',
    props: {
      action: "An action after the rows, such as a button that opens every right.",
      "aria-label": "The accessible name of the list.",
      groups: "The same groups as RightsMatrix; only `granted`, `label` and `code` are read.",
      sources: "What the access comes from, as chips above the rows: the member's roles.",
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
    description: "A redacted key: mono 13 with its state dot, copy of the redacted form, and its access.",
    props: {
      access: '"Inference", "Platform", or both.',
      copy: "Hide the copy circle.",
      redacted: "The redacted key as the API returns it: sk-kso-AbC123…wxyz",
      state: "The key's state; disabled, expired and archived keys are shown dimmed.",
    },
  },
  LimitMeter: {
    description: "A limit: its kind, scope and window, hard or soft, the meter with alert ticks, and when it resets.",
    props: {
      alerts: "Alert thresholds as ratios.",
      hard: "Whether requests are refused at the limit (`true`) or only alerted (`false`).",
      kind: "What is limited: requests, tokens or spend, and over which period.",
      limit: "The limit.",
      resets: '"Resets in 12 days".',
      scope: '"Project", "Key ci-runner", "Member Ada".',
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
      model: "The model id, in mono: after the name, quieter, or alone when there's no name.",
      name: "The display name, first.",
      provider: "The provider id, such as `openai` or `anthropic`.",
    },
  },
  ProviderBadge: {
    description:
      "A provider's mark on a quiet tile, in the provider's own one-colour version for the theme; a letter for providers without a mark or whose guidelines don't allow one.",
    props: {
      provider: "The provider id, such as `openai` or `anthropic`.",
      size: "The badge's size.",
      withName: "Writes the provider's name next to its mark.",
    },
  },
  ProviderCard: {
    description: "A provider and how this place reaches it: its mark, name and connection, the key in use, and the actions to connect, replace or remove one.",
    props: {
      actions: "Buttons: one primary at most.",
      connected: "Whether requests can reach the provider.",
      detail: "A line under the provider's name, such as how many keys it has.",
      keyLabel: "The key: its name and last four characters, and when it was last used.",
      provider: "The provider id, such as `openai` or `anthropic`.",
      status: 'Where the key comes from, or what\'s missing: "The organization\'s key", "No key yet".',
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
      'Rights by group, for a role, a member or a key. Edit mode: a checkbox per right and per group (mixed when some are granted), requirements kept (granting "edit" grants "view"; withdrawing "view" withdraws "edit"), each right\'s needs said under it, and arrow keys moving between checkboxes. Read-only mode: a mark per right and the roles that grant it. Rows keep their size whatever their state. Fits a sheet and a phone.',
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
      "Choosing a role: built-in roles first, then custom ones under their own caption, each with its one-sentence description. A role the person can't give stays in the list, disabled, with the reason under it and in its tooltip, so nobody wonders where it went.",
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
      'The one-time secret: calm, centered, certain. The secret on a well (selected on click), a large "Copy key" that becomes "Copied" without changing size, a plain warning, and snippets that use it.',
    props: {
      apiKey: "What stands in for the key: the real key right after creation, a placeholder elsewhere.",
      baseUrl: "The gateway's address, for the code snippet that uses the key.",
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
    description: "A stacked bar of token kinds with the numbers under it, in the fixed series mapping.",
    props: {
      tokens: "Tokens by kind: input, output, cache reads and writes, reasoning.",
    },
  },
} satisfies ComponentDocs;
