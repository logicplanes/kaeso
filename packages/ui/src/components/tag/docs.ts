import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Pill: {
    description: "Another name for Tag.",
    props: {
      children: "The pill's text.",
      icon: "An icon before the text.",
      invalid: "Marks a value that isn't accepted.",
      onClick: "Renders a button with a hover fill.",
      onRemove: "Shows a remove circle; called with the click.",
      pressed: "For toggle pills (DisplayOptions).",
      removeLabel: "The remove circle's accessible name. Defaults to \"Remove\" and the label in the provider's language.",
      size: "The pill's height.",
      variant: "Raised on the panel, soft fill, or outlined.",
    },
  },
  Tag: {
    description:
      "A metadata pill: 28px, 13/500, an optional icon, an optional remove circle. A label longer than its room (a long key or model name) ends in an ellipsis and shows in full in a tooltip; give a short status label the room it needs instead (a column at least as wide as the pill).",
    props: {
      children: "The tag's text.",
      icon: "An icon before the text.",
      invalid: "Marks a value that isn't accepted.",
      onClick: "Renders a button with a hover fill.",
      onRemove: "Shows a remove circle; called with the click.",
      pressed: "For toggle pills (DisplayOptions).",
      removeLabel: "The remove circle's accessible name. Defaults to \"Remove\" and the label in the provider's language.",
      size: "The tag's height.",
      variant: "Raised on the panel, soft fill, or outlined.",
    },
  },
} satisfies ComponentDocs;
