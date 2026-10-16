import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Banner: {
    description:
      "An inline notice: a tint, an icon, an optional title, the text, actions at the end. The banner is its own size container. From 480px inside its padding (the `banner` container size, `@banner:`) the actions sit on the right, centered on the title and text together, and the icon stays on the first line. Narrower (a phone) the actions go under the text, starting where the text starts: their 24px inset is the icon's 16px column and its 8px gap, which in the row is the gap before them.",
    props: {
      actions: "sm pills: on the right, centered on the title and text; under the text when the banner is narrow.",
      children: "The message, with actions if it needs them.",
      icon: "An icon in place of the tone's own.",
      layout: "Inside the content (`inline`) or across the full width of the panel (`full`).",
      onDismiss: "Only informational banners can be dismissed.",
      title: "The banner's heading.",
      tone: "The kind of message, from neutral information to a danger warning.",
    },
  },
} satisfies ComponentDocs;
