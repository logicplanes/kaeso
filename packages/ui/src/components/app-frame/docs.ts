import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  AppFrame: {
    description:
      "The page frame of an app: a skip link to the content, the sidebar, the inset panel and, on phones, a bar with a menu button. The navigation sheet's code loads once the page is idle or on the first tap; after a failed download the next tap or hover loads it again from a fresh address. Chromium and WebKit keep a failed download of a chunk the sheet shares with the rest of the app until the page is reloaded.",
    props: {
      brandHref: "Where the phone bar's wordmark links to (the app's home), as `Sidebar.Brand`'s `href`; without it the wordmark is not a link.",
      brandLabel: "The accessible name of the phone bar's wordmark (default \"Kaeso\"), as `Sidebar.Brand`'s `label`.",
      children: "The page, inside the inset panel.",
      corner: "A bottom-right status area. Unused by default.",
      fill: "`screen` fills the viewport (default); `parent` fills its container (previews, embedded tools).",
      phoneActions: "Icon buttons in the phone bar before the navigation toggle, such as search, so it is one tap away; pass `tooltip={false}` to each.",
      sidebar: "The Sidebar.",
    },
  },
  NavSheet: {
    description:
      "The sidebar as a floating panel from the left edge (a peek): the mobile navigation sheet, and the preview of a collapsed sidebar while the expand button is hovered (`peek`: it closes when the pointer leaves).",
    props: {
      children: "The navigation, usually the app's `Sidebar`.",
      onOpenChange: "Called when the navigation opens or closes.",
      open: "Whether the navigation is shown on phones.",
      peek: "Slides the sheet in a little, to hint that it can be opened.",
      returnFocus: "The element that gets focus back when the sheet closes.",
    },
  },
} satisfies ComponentDocs;
