import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  AgentAvatar: {
    description:
      "An agent acting for a person: one bot face on an ink square, the same for every agent; write its model family beside it with useModelName where it matters.",
    props: {
      decorative: "Hides it from assistive technology when the agent's name is already written next to it.",
      name: "The agent's name, such as `Release helper`; it is the accessible name.",
      size: "The side of the square in pixels, on the same scale as `Avatar`.",
    },
  },
  AgentAttribution: {
    description:
      "A person's avatar with the agent that acted for them as a small badge on its corner, for audit rows and activity; it reads as one picture named after both. The badge reaches up to 8px past the avatar's corner, so leave 12px beside it.",
    props: {
      agent: "The agent that acted, with its name and model family id from the API; the family is named in the accessible name only.",
      name: "The name of the person the agent acted for; it starts the accessible name, such as `Ada Okafor via Release helper (Claude)`.",
      size: "The diameter of the person's avatar in pixels; the agent badge is half of it.",
      src: "The person's picture; without one, or when it fails to load, their initials show.",
    },
  },
} satisfies ComponentDocs;
