import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Icon: {
    description:
      "The one way to draw an icon: Octicons' 16px drawings in currentColor, whose 1.5px lines match the weight of the text beside them. When the icon changes, as Copy turns to Copied, the old one shrinks out and the new one grows in.",
    props: {
      icon: "An icon from `icons` in `@logicplanes/kaeso-ui`, or one of the drawings in `@logicplanes/kaeso-ui/octicons`.",
      label: "When set, the icon is announced with this name; otherwise it is decorative and hidden.",
      size: "The icon's size on the type scale.",
    },
  },
} satisfies ComponentDocs;
