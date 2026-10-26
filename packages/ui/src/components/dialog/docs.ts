import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  AlertDialog: {
    description: "Confirms an irreversible action: names it, explains the consequence, optionally asks for typed confirmation.",
    props: {
      cancelLabel: 'Defaults to "Cancel" in the provider\'s language.',
      confirmLabel: 'The confirm label, the verb and object: "Delete key".',
      confirmText: "Typed confirmation: the person types this to enable the confirm.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      description: "The practical consequence.",
      inline: "Renders in place, without a trigger, for review pages and tests.",
      loading: "Shows a spinner on the confirm while work runs (also automatic for a promise); until it ends, the dialog stays open.",
      onConfirm:
        "Called when the action is confirmed; return a promise to keep the dialog busy until it ends. It closes when the promise resolves and stays open for another try when it rejects, so toast the error there.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      title: 'Names the action and the object: "Delete “checkout-web”?"',
      tone: "How serious the confirmation is: `danger` for an action that can't be undone (a danger confirm, the focus on Cancel), `warning` for a risky one that can (a primary confirm, the focus still on Cancel), `info` for a neutral one (the focus on the confirm).",
      trigger: "The element that opens it.",
    },
  },
  Dialog: {
    description:
      'A modal dialog: 440 / 560 / 720, radius 20, title, description, body, actions. Enter in a single-line field and ⌘/Ctrl+Enter anywhere submit; Enter in a text area adds a line. It stays centered (or, with `anchor="top"`, keeps its top edge) while its content changes, growing or shrinking 200ms from the size it has; the body scrolls between the header and the footer when the content is taller than the screen. Its content unmounts once it has closed; state kept beside it, like a field, a step or a secret, goes in a component inside a DialogScope.',
    props: {
      anchor: "Where it sits: centered, or with a fixed top edge for content that changes height, so the title never moves.",
      busy: "Keeps the dialog open while work it started is underway: Escape, a press outside, the close circle, Dialog.Close, Enter and ⌘Enter do nothing, so a request is never abandoned or sent twice.",
      children: "The dialog's content and actions.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      description: "One sentence under the title.",
      footer: "Right-aligned actions: Cancel (secondary) then the primary.",
      footerStart: "A quiet action at the footer's start, such as a link to another way of doing the task.",
      toolbar: "Controls that stay under the title while the body scrolls, such as a search over the list below; a hairline shows once the body is scrolled.",
      headerStart:
        "A top bar above the title, with this at its start and the close circle at its end (LayerDialog's back button). The title, description and content keep one left edge; `null` keeps the bar with an empty start.",
      hideClose: "Hide the close circle (rare).",
      inline: "Render without a portal and backdrop, for documentation and state matrices.",
      onOpenChange: "Called when it opens or closes.",
      onSubmit:
        "Enter in a single-line field and ⌘/Ctrl+Enter anywhere call this (usually the primary action); Enter in a text area, a search field, a combobox or a picker, and the Enter that confirms an input method's text, keep their own meaning.",
      open: "Whether it is open, when you control it.",
      persistent:
        "A press outside doesn't close it or move the focus, for forms that would lose what was typed; Escape and the close circle still close it. Every StepDialog is persistent.",
      size: "The dialog's width.",
      title: "The dialog's heading, also its accessible name; a long one, or an id, wraps.",
      trigger: "The trigger, a single element.",
    },
  },
  DialogScope: {
    description:
      "Opens a dialog's flow fresh every time: its children mount anew when `open` turns true and unmount once the Dialog, StepDialog, LayerDialog, AlertDialog, Sheet or CommandPalette inside has finished closing. Fields, steps, errors, results and secrets shown once never come back on the next open, and nothing changes while the dialog animates out.",
    props: {
      children: "The component that keeps the flow's state and renders the dialog with the same `open`.",
      open: "Whether the dialog inside is open; pass the value its `open` gets.",
    },
  },
  LayerDialog: {
    description:
      "A stacked dialog: a second layer slides over the first (add model → review values); Back, or Escape, returns. The layers below stay as they were. The close circle sits in a bar above the title, with Back at its start from the second layer, so the title keeps its place on every layer. Moving to a layer puts the focus on its first field, or its primary action; Back returns it to the control that opened the layer, scrolled where it was. Enter in a single-line field and ⌘/Ctrl+Enter call onSubmit on every layer.",
    props: {
      anchor: "Where it sits: with a fixed top edge (default), so the title stays put on layers of different heights, or centered.",
      busy: "Keeps the dialog open while work it started is underway: Escape, a press outside, the close circle, Dialog.Close, Enter and ⌘Enter do nothing, so a request is never abandoned or sent twice.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      hideClose: "Hide the close circle (rare).",
      inline: "Render without a portal and backdrop, for documentation and state matrices.",
      layer: "The visible layer; layers above the first slide over it.",
      layers:
        "The layers, each with its title, content and footer, and optionally a toolbar and a footer start; one shows at a time, and the ones below it stay mounted and hidden.",
      onLayerChange: "Called with the index of the layer shown.",
      onOpenChange: "Called when it opens or closes.",
      onSubmit: "Enter in a single-line field and ⌘/Ctrl+Enter call this on every layer; pass the action of the layer shown.",
      open: "Whether it is open, when you control it.",
      persistent:
        "A press outside doesn't close it, for forms that would lose what was typed; Escape and the close circle still do. Every StepDialog is persistent.",
      size: "The dialog's width.",
      trigger: "The trigger, a single element.",
    },
  },
  StepDialog: {
    description:
      "A form split into steps, one question at a time, like sign-in: the step's title and fields, Back and Continue, the action on the last step. Enter in a single-line field and ⌘/Ctrl+Enter continue. The dialog takes each step's own height and keeps its top edge where it opened: it grows or shrinks 200ms downward from the size it has as the step changes, so the title never jumps, and the new step drifts in from the side it comes from. With `result`, what the last step made (a key shown once) takes the steps' place in the same dialog, at its own size. Continue and Back move the focus into the step they show (its first field; without one, the primary action, or Back when the last step destroys), and a screen reader hears which step it is. Inside a step, `useStepDialog()?.advance()` continues as Continue would, once the step is no longer blocked: a choice that answers the step, like a provider tile, moves on in one press.",
    props: {
      defaultStep: "Start on this step (state matrices).",
      inline: "Render without a portal and backdrop, for documentation and state matrices.",
      onOpenChange: "Called when it opens or closes.",
      onSubmit: "Called when the last step is submitted.",
      open: "Whether it is open, when you control it.",
      result:
        "What the last step made, shown in the same dialog once it exists: its title, description, content, footer and size. Pass it after `onSubmit` succeeds, such as the created key with SecretReveal.",
      size: "The dialog's width.",
      steps:
        'The steps, each with its title, content and whether it can be continued: with `blockedReason`, Continue waits, and Enter shows and announces the reason under the step; `onContinue` checks a step before the next opens, such as asking a server, and `continueLabel` names that check: "Detect settings".',
      submitLabel: 'The last step\'s button: "Create key".',
      submitting: "Shows that the submission is underway; until it ends, the dialog stays open and Back and Cancel wait.",
      title: 'What the whole dialog does: "Create key".',
      tone: "`danger` for an action that destroys something: the last step's button turns red.",
    },
  },
} satisfies ComponentDocs;
