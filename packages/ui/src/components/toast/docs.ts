import type { ComponentDocs } from "../../component-docs";

export const docs = {
  Toast: {
    description: "`<Toast />` mounts the toaster like `Toaster`, and `Toast.add()` shows a toast like `toast()`.",
    props: {
      limit: "How many toasts show at once.",
      timeout: "How long a toast stays, in milliseconds.",
    },
  },
  ToastCapsule: {
    description: "The capsule alone, for documentation and state matrices.",
    props: {
      action: "The label of the toast's action button.",
      closable: "Shows a close button.",
      description: "One more sentence, when the message needs it.",
      title: "The message.",
      tone: "The kind of message; `loading` shows a spinner.",
    },
  },
  Toaster: {
    description:
      "Mount once near the root. Three visible at most; 5s, paused on hover and focus; swipe or Escape to dismiss. With a client router that replaces <body> on navigation (Astro's ClientRouter), mount it in a persisted element inside a `PortalContainer`, so the toasts are not swapped away with the page.",
    props: {
      limit: "How many toasts show at once.",
      timeout: "How long a toast stays, in milliseconds.",
    },
  },
} satisfies ComponentDocs;
