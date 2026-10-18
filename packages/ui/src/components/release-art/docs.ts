import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ReleaseArt: {
    description:
      "Abstract artwork drawn from a seed, such as a version: flowing lines and a soft glow in ink, the same for the same seed and different for every other, with an optional title set on it.",
    props: {
      seed: "What the artwork is drawn from, e.g. `0.3.0`; the same seed always draws the same picture.",
      label: 'What the picture shows, read by screen readers: "Kaeso 0.3.0".',
      title: "Large text on the artwork, e.g. the version number.",
      eyebrow: "Small text above the title, e.g. “What's new in Kaeso”.",
      shape: "The artwork's aspect ratio.",
    },
  },
} satisfies ComponentDocs;
