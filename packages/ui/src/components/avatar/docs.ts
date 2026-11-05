import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Avatar: {
    description:
      'The first letters of the first and last word; `max` 1 keeps only the first. A photo gets a hairline so a light picture keeps its edge. An agent (`kind="agent"`) is one bot face on a `fill-3` square, the same for every agent: people are round, agents square. With `agent`, a person\'s avatar carries the agent that acted for them as a small round ink badge on its corner, for audit rows and activity; it reads as one picture named after both, and a tooltip says the same to the pointer.',
    props: {
      agent:
        "The agent that acted for this person, with its name and model family id from the API: a badge of about 40% of the avatar on its corner, and the family in the accessible name, such as `Ada Okafor via Release helper (Claude)`.",
      decorative: "Hides it from assistive technology when the same information is already written next to it.",
      kind: "A person (round), an organization (rounded square), or an agent (a bot face on a square).",
      name: "The person's, organization's or agent's name; its first letters show without a picture, and it is the accessible name.",
      size: "The diameter in pixels.",
      src: "The picture's address; without one, or when it fails to load, the letters show.",
    },
  },
  "Avatar.Group": {
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
