import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Avatar: {
    description: "The first letters of the first and last word; `max` 1 keeps only the first. A photo gets a hairline so a light picture keeps its edge.",
    props: {
      decorative: "Hides it from assistive technology when the same information is already written next to it.",
      kind: "A person (round) or an organization (rounded square).",
      name: "The person's or organization's name; its first letters show without a picture, and it is the accessible name.",
      size: "The diameter in pixels.",
      src: "The picture's address; without one, or when it fails to load, the letters show.",
    },
  },
  AvatarGroup: {
    description:
      'Overlapping avatars, each separated from the one under it by a 2px gap cut out of that avatar, so the gap shows whatever surface the group sits on (a hovered row, a card); then a "+3".',
    props: {
      children: "Content after the avatars, such as a button to invite more people.",
      max: 'Show at most this many; the rest become "+3", which names them in a tooltip.',
      people: "The people to show, in order.",
      size: "The diameter of each avatar in pixels.",
    },
  },
} satisfies ComponentDocs;
