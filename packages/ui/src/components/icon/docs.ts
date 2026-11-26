import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Icon: {
    description:
      "The one way to draw an icon: Lucide in currentColor, with a 1.5px stroke at every size to match the weight of the text beside it. When the icon changes, as Copy turns to Copied, the old one shrinks out and the new one grows in.",
    props: {
      icon: "A Lucide icon, from `lucide` or `icons` in `@logicplanes/kaeso-ui`.",
      label: "When set, the icon is announced with this name; otherwise it is decorative and hidden.",
      size: "The icon's size on the type scale.",
    },
  },
} satisfies ComponentDocs;
