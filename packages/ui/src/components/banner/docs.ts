import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Banner: {
    description:
      "An inline notice: a tint, an icon (none for a neutral notice, so it never looks like information), an optional title, the text, actions at the end. Inline, the icon sits 12px in and the banner is at least 56px tall, so banners with and without actions stack evenly; full width, the icon sits on the 24px gutter and the text 24px further in. The banner is its own size container. From 480px inside its padding (the `banner` container size, `@banner:`) the actions sit on the right, centered on the title and text together, and the icon stays on the first line. Narrower (a phone) the actions go under the text, starting where the text starts: their 24px inset is the icon's 16px column and its 8px gap, which in the row is the gap before them.",
    props: {
      actions: "sm pills: on the right, centered on the title and text; under the text when the banner is narrow.",
      children: "The message, with actions if it needs them.",
      icon: "An icon in place of the tone's own; a neutral banner has none unless you pass one.",
      layout: "Inside the content (`inline`) or across the full width of the panel (`full`).",
      onDismiss:
        "Shows a dismiss button on `neutral` and `info` banners. Success, warning and danger banners stay until their cause changes, so they ignore it and warn in development.",
      title: "The banner's heading.",
      tone: "The kind of message, from neutral information to a danger warning.",
    },
  },
} satisfies ComponentDocs;
