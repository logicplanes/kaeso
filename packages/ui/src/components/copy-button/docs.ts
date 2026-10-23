import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ClipboardText: {
    description: "A read-only pill field: the value in mono and a copy circle inside.",
    props: {
      display: "Shown instead of the value (for example a redacted form); the value is still what is copied.",
      label: 'The tooltip and accessible name. Defaults to "Copy" in the provider\'s language.',
      size: "The height of the field.",
      value: "The text to show and copy.",
    },
  },
  CopyButton: {
    description: 'A circle that copies a value: a check and "Copied" for 1.2s.',
    props: {
      label: 'What is being copied, for the accessible name: "Copy key". Defaults to "Copy" in the provider\'s language.',
      onCopy: "Called with the copied text.",
      size: "The button's height.",
      value: "The text that is copied.",
      variant: "A filled button (`secondary`) or a quiet one (`ghost`).",
    },
  },
  InlineCopyText: {
    description:
      "Inline mono text that copies itself on click. On hover and keyboard focus a copy icon appears after it. When nothing follows it on its line and its row's padding has room, the icon takes no space, so the text ends on the same edge as the values around it; otherwise the icon keeps its own space. Touch screens always show the icon in its own space. Never wider than its container: a long value (a user agent, an id) ends in an ellipsis, and its tooltip shows the whole of it until it is copied.",
    props: {
      children: "What is shown, when it differs from the copied text.",
      label: 'The tooltip and accessible name. Defaults to "Copy" in the provider\'s language.',
      onCopy: "Called with the copied text.",
      value: "The text that is copied.",
    },
  },
} satisfies ComponentDocs;
