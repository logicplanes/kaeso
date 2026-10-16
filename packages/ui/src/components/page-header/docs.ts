import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  PageHeader: {
    description:
      "The 52px header of a page: its title, pill tabs, actions. On a phone the tabs move to a row of their own under the title and actions, so the title isn't cut to a few letters and the tabs aren't squeezed beside a button.",
    props: {
      actions: "At most one ink button; the rest ghost or raised circles with tooltips.",
      icon: "An icon before the title.",
      start:
        "Before everything: AppFrame.MenuButton, which shows while a wide screen's sidebar is collapsed (on a phone the frame's phone bar opens the navigation).",
      tabs: "Pill tabs after the title.",
      title: "The page's title (15/600).",
    },
  },
} satisfies ComponentDocs;
