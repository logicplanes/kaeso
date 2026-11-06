import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  SettingsGroup: {
    description: "A group of settings rows: radius 14, raised, ring and a whisper of shadow; hairlines inset to the text.",
    props: {
      "aria-label": "The group's accessible name.",
      children: 'The rows of the group: `SettingsRow`s, a `RadioGroup` with `variant="cards"`, or `Checkbox`es with `variant="card"`, which share its surface.',
    },
  },
  SettingsRow: {
    description:
      "One setting: title 15/500 and description 13 on the left, the control on the right. The row is its own size container: narrower than 480px inside its padding (the `settings-row` container size, a phone), every row's control moves under its text, so the rows of a group line up the same way whatever their control's width. A ghost or quiet button that ends the control keeps its label on the row's edge, where an outlined button's ring ends; under the text it starts on the text's edge.",
    props: {
      control: "The control on the right: Select sm, Switch, a ghost button, a link with ↗.",
      danger: "Marks a destructive setting.",
      description: "One sentence about the setting.",
      href: "Makes the row a link to this address.",
      icon: "An 18px icon at the left, in secondary: no tile behind it.",
      id: "For the label of the control.",
      onClick: "Makes the row a button and is called when it is pressed.",
      title: "The setting's name.",
      value: "A value in secondary before a chevron: makes the row a navigation row.",
    },
  },
} satisfies ComponentDocs;
