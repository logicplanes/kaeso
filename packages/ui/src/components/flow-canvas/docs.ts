import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  FlowCanvas: {
    description:
      "An editable route drawn as a tree, top down: the start, then each step with its branches side by side under it, and a model's fallback continuing its row to the right, so a chain reads in the order its models are tried. The layout is computed, never measured, so it is the same at every zoom. Drag the background or scroll to pan; ⌘ or Ctrl with the wheel, a pinch or the buttons at the bottom left zoom between 25% and 150%; it fits itself once, on first render, never below 67% so every step stays readable, and then keeps the view when the route changes. The steps are one tree for the keyboard and screen readers.",
    props: {
      addMenu:
        "Turns editing on: open slots and a + midway on every connection show, and each opens a menu with these items. Called with the connection's `edge` id and `insert`: false for an open slot, true for the + that puts a step before the connection's step. Without it, open slots are not drawn.",
      "aria-label": 'The tree\'s accessible name: what the route belongs to, such as "Route of fast".',
      background: "A dotted grid that moves with the view, or the plain panel.",
      nodeActions:
        "Menu items for a step, shown in a button on the step and opened with Shift F10 or the context-menu key. Return nothing for a step without actions, such as the start.",
      onDelete: "Called with a step's id when Delete or Backspace is pressed on it.",
      onSelect: "Called with a step's id when it is clicked or chosen with Enter or Space, and with null when the background is clicked or Escape is pressed.",
      root: "The start of the route. Each node has `kind` (its tag), `title`, an optional `subtitle`, `leading` (such as a provider badge), `status` (a word and a tone), `badge` (a short word at its top right, such as its try), `terminal` (a model: its one connection is its fallback, drawn to its right) and `children`: connections with an `id`, a `label`, a `variant` (`fallback` draws a dashed line) and the `node` they lead to; a connection without a node is an open slot.",
      selected: "The id of the selected step, drawn with the selection ring; the view moves to show it when it changes.",
      trace:
        "The ids of the steps one request passes, such as from a dry run: they and the lines between them stay strong, and every other step turns quieter: secondary text and a hairline, still readable. Leave it out to draw the route evenly.",
    },
  },
  FlowWorkspace: {
    description:
      "The page body of an editor built on a `FlowCanvas`: the canvas fills the panel under the page header, and an inspector column on the right edits what is selected. From `lg` the inspector is a column of its own that scrolls on its own; below `lg` it sits under a 400px canvas and the page scrolls.",
    props: {
      children: "The canvas.",
      panel: "The inspector: usually a `FlowWorkspace.Panel`.",
      panelLabel: 'The inspector\'s accessible name as a region, such as "Step settings".',
      width: "The inspector's width from `lg`.",
    },
  },
  "FlowWorkspace.Panel": {
    description: "One inspector section on the 24px gutter: a title with actions beside it, a line of description, then the fields.",
    props: {
      actions: "Buttons beside the title, such as Remove step.",
      children: "The fields, 20px apart.",
      description: "One sentence under the title.",
      title: "The section's heading.",
    },
  },
} satisfies ComponentDocs;
