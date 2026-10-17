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
      loading: "Shows a spinner on the confirm while work runs (also automatic for a promise).",
      onConfirm:
        "Called when the action is confirmed; return a promise to keep the dialog busy until it ends. It closes when the promise resolves and stays open for another try when it rejects, so toast the error there.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      title: 'Names the action: "Delete this key?"',
      tone: "How serious the confirmation is; `danger` for actions that can't be undone.",
      trigger: "The element that opens it.",
    },
  },
  Dialog: {
    description:
      "A modal dialog: 440 / 560 / 720, radius 20, title, description, body, actions. ⌘Enter submits. It stays centered while its content changes, growing or shrinking 200ms from the size it has; the body scrolls between the header and the footer when the content is taller than the screen. Its content unmounts once it has closed; state kept beside it, like a field, a step or a secret, goes in a component inside a DialogScope.",
    props: {
      children: "The dialog's content and actions.",
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      description: "One sentence under the title.",
      footer: "Right-aligned actions: Cancel (secondary) then the primary.",
      headerStart:
        "A top bar above the title, with this at its start and the close circle at its end (LayerDialog's back button). The title, description and content keep one left edge; `null` keeps the bar with an empty start.",
      hideClose: "Hide the close circle (rare).",
      inline: "Render without a portal and backdrop, for documentation and state matrices.",
      onOpenChange: "Called when it opens or closes.",
      onSubmit: "⌘Enter calls this (usually the primary action).",
      open: "Whether it is open, when you control it.",
      size: "The dialog's width.",
      title: "The dialog's heading, also its accessible name.",
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
      "A stacked dialog: a second layer slides over the first (add model → review values); back returns. The layers below stay as they were. Moving to a layer puts the focus on its first field, or its primary action; Back returns it to the control that opened the layer, scrolled where it was. ⌘/Ctrl+Enter calls onSubmit on every layer.",
    props: {
      defaultOpen: "Whether it starts open, when the component keeps its own state.",
      hideClose: "Hide the close circle (rare).",
      inline: "Render without a portal and backdrop, for documentation and state matrices.",
      layer: "The visible layer; layers above the first slide over it.",
      layers: "The layers, each with its title, content and footer; one shows at a time, and the ones below it stay mounted and hidden.",
      onLayerChange: "Called with the index of the layer shown.",
      onOpenChange: "Called when it opens or closes.",
      onSubmit: "⌘/Ctrl+Enter calls this on every layer; pass the action of the layer shown.",
      open: "Whether it is open, when you control it.",
      size: "The dialog's width.",
      trigger: "The trigger, a single element.",
    },
  },
  StepDialog: {
    description:
      "A form split into steps, one question at a time, like sign-in: the step's title and fields, Back and Continue, the action on the last step. Enter in a field and ⌘/Ctrl+Enter continue. The dialog takes each step's own height and stays centered: it grows or shrinks 200ms from the size it has as the step changes, and the new step drifts in from the side it comes from. With `result`, what the last step made (a key shown once) takes the steps' place in the same dialog, at its own size. Continue and Back move the focus into the step they show (its first field; without one, the primary action, or Back when the last step destroys), and a screen reader hears which step it is.",
    props: {
      defaultStep: "Start on this step (state matrices).",
      inline: "Render without a portal and backdrop, for documentation and state matrices.",
      onOpenChange: "Called when it opens or closes.",
      onSubmit: "Called when the last step is submitted.",
      open: "Whether it is open, when you control it.",
      result:
        "What the last step made, shown in the same dialog once it exists: its title, description, content, footer and size. Pass it after `onSubmit` succeeds, such as the created key with SecretReveal.",
      size: "The dialog's width.",
      steps: "The steps, each with its title, content and whether it can be continued.",
      submitLabel: 'The last step\'s button: "Create key".',
      submitting: "Shows that the submission is underway.",
      title: 'What the whole dialog does: "Create key".',
      tone: "`danger` for an action that destroys something: the last step's button turns red.",
    },
  },
} satisfies ComponentDocs;
