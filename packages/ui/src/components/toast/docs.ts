import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Toast: {
    description: "`<Toast />` mounts the toaster like `Toaster`, and `Toast.add()` shows a toast like `toast()`.",
    props: {
      limit: "How many toasts show at once.",
      timeout: "How long a toast stays, in milliseconds; errors and toasts with an action stay until dismissed, unless `toast()` gets a timeout of its own.",
    },
  },
  ToastCapsule: {
    description: "The capsule alone, for documentation and state matrices.",
    props: {
      action: "The label of the toast's action button.",
      closable: "Shows the close button at its end, as a toast that stays until dismissed has.",
      description: "One more sentence, when the message needs it.",
      title: "The message.",
      tone: "The kind of message; `loading` shows a spinner.",
    },
  },
  Toaster: {
    description:
      "Mount once near the root. Three visible at most; 5s, paused on hover and focus; an error, or a toast with an action such as Undo, stays until it is dismissed; the close button, a swipe or Escape dismisses, and F6 moves the focus to the toasts. With a client router that replaces <body> on navigation (Astro's ClientRouter), mount it in a persisted element inside a `PortalContainer`, so the toasts are not swapped away with the page.",
    props: {
      limit: "How many toasts show at once.",
      timeout: "How long a toast stays, in milliseconds; errors and toasts with an action stay until dismissed, unless `toast()` gets a timeout of its own.",
    },
  },
} satisfies ComponentDocs;
