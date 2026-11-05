import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Button: {
    description: "The button: a pill with a label, or, with `icon` and no label, a circle that shows its `aria-label` as a tooltip.",
    props: {
      children: "The button's label: a verb first, in sentence case. A label wider than its container is cut with an ellipsis and shown whole in a tooltip.",
      count: "A count after the label.",
      disabledReason:
        "Why the button is disabled, shown as a tooltip and read out as the button's description. The button stays focusable so people can reach it.",
      fullWidth: "Fills the width of its container.",
      href: "Renders a link that looks like the button.",
      icon: "Makes it an icon-only circle with this icon. Give it an `aria-label`: the name screen readers read and the tooltip shows.",
      leadingIcon: "An icon before the text.",
      loading:
        "Work is underway: the button is busy and ignores clicks at once; the spinner shows only once the work has taken 300ms, then for at least 400ms (`useDelayedLoading`). It replaces the leading icon, or covers the label; the width never changes.",
      rel: "The link's relationship, as on an `a` element.",
      reserveLabels: 'Every label this button can show ("Copy key", "Copied"). The button keeps the width of the widest, so a state change never resizes it.',
      shortcut: "A shortcut hint: after the label (lg and xl, and in menus), or in the tooltip of an icon-only button.",
      size: "The height, from `xs` (24px) to `xl` (48px).",
      static: "Keeps the button still when pressed, for a control whose 0.96 press would distract; it still darkens.",
      target: "Where the link opens, as on an `a` element.",
      tooltip: "Turns off an icon-only button's tooltip, for when a visible label sits next to it or the button has a tooltip of its own.",
      tooltipSide: "Where an icon-only button's tooltip appears.",
      trailingIcon: "An icon after the text.",
      variant:
        "`primary` for the one main action of a view, `secondary` for others, `ghost` and `quiet` for toolbars and rows, `danger` for destructive actions. A ghost or quiet button at the end of a settings row's control pulls its padding past the row's edge, so its label ends where an outlined button's ring does.",
    },
  },
} satisfies ComponentDocs;
