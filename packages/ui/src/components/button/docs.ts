import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Button: {
    description: "The button. Every one is a pill; icon-only buttons are circles (see `IconButton`).",
    props: {
      children: "The button's label: a verb first, in sentence case.",
      count: "A count after the label.",
      disabledReason: "Why the button is disabled, shown as a tooltip. The button stays focusable so people can read it.",
      fullWidth: "Fills the width of its container.",
      href: "Renders a link that looks like the button.",
      icon: "The only content of an icon button; give it an `aria-label`.",
      leadingIcon: "An icon before the text.",
      loading:
        "Work is underway: the button is busy and ignores clicks at once; the spinner shows only once the work has taken 300ms, then for at least 400ms (`useDelayedLoading`). It replaces the leading icon, or covers the label; the width never changes.",
      rel: "The link's relationship, as on an `a` element.",
      reserveLabels: 'Every label this button can show ("Copy key", "Copied"). The button keeps the width of the widest, so a state change never resizes it.',
      shape: "Icon-only buttons are circles and need an accessible name.",
      shortcut: "A shortcut hint after the label (lg and xl, and in menus).",
      size: "The height, from `xs` (24px) to `xl` (44px).",
      target: "Where the link opens, as on an `a` element.",
      trailingIcon: "An icon after the text.",
      variant:
        "`primary` for the one main action of a view, `secondary` for others, `ghost` and `quiet` for toolbars and rows, `danger` for destructive actions. A ghost or quiet button at the end of a settings row's control pulls its padding past the row's edge, so its label ends where an outlined button's ring does.",
    },
  },
} satisfies ComponentDocs;
