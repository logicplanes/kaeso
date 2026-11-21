import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  PageHeader: {
    description:
      "The 52px header of a page: its title, pill tabs, actions. Beside the tabs, a long title is cut short first, down to nothing if need be; only then do the tabs scroll. Whatever sits right above AppFrame.Body (this header, or a toolbar under it) shows a hairline at its foot while the content is scrolled under it, so the line separates only where something passes beneath. On a phone the tabs move to a row of their own under the title and actions, and actions that don't fit beside the whole title move to a row of their own too, wrapping onto another line when they are wider than the page, so the title isn't cut short, the tabs aren't squeezed beside a button and no action runs off the edge.",
    props: {
      actions: "At most one ink button; the rest ghost or raised circles with tooltips.",
      icon: "An icon before the title.",
      start:
        "Before everything: AppFrame.MenuButton, which shows while a wide screen's sidebar is collapsed (on a phone the frame's phone bar opens the navigation).",
      tabs: "Pill tabs after the title.",
      title: "The page's title (17/600), alone: no label above it.",
    },
  },
} satisfies ComponentDocs;
