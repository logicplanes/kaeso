import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Toaster: {
    description:
      "Mount once near the root. Three visible at most; 5s, paused on hover and focus; an error, or a toast with an action such as Undo, stays until it is dismissed; the close button, a swipe or Escape dismisses, and F6 moves the focus to the toasts. With a client router that replaces <body> on navigation (Astro's ClientRouter), mount it in a persisted element inside a `PortalContainer`, so the toasts are not swapped away with the page.",
    props: {
      limit: "How many toasts show at once.",
      timeout: "How long a toast stays, in milliseconds; errors and toasts with an action stay until dismissed, unless `toast()` gets a timeout of its own.",
    },
  },
} satisfies ComponentDocs;
