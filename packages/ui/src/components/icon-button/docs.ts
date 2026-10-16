import type { ComponentDocs } from "../../component-docs";

export const docs = {
  IconButton: {
    description: "A circle with an icon, a required name, and a tooltip that says it.",
    props: {
      "aria-label": "The accessible name, also shown as the tooltip. Required.",
      disabledReason: "Why it is disabled, shown as a tooltip; it stays focusable so people can read it.",
      href: "Makes it a link to this address.",
      icon: "The icon; give the button an `aria-label`.",
      loading: "Shows that work is underway and ignores clicks until it ends.",
      shortcut: "A shortcut shown in the tooltip.",
      size: "The diameter, from `xs` (24px) to `xl` (44px).",
      tooltip: "Turn the tooltip off (rare: when a visible label sits next to it).",
      tooltipSide: "Where the tooltip with its name appears.",
      variant: "The look, as on `Button`.",
    },
  },
} satisfies ComponentDocs;
