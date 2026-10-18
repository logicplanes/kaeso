import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  ReleaseArt: {
    description:
      "A release's artwork: flat geometric art in one of eight styles and Kaeso's palettes, drawn from a stored art spec or a seed, with an optional title floating on it.",
    props: {
      seed: "What the artwork is drawn from when there is no `art`, e.g. `0.3.0`; the same seed always draws the same picture.",
      art: "The release's stored artwork, e.g. from `nextReleaseArt`; it always draws the same picture and wins over `seed`.",
      label: 'What the picture shows, read by screen readers: "Kaeso 0.3.0".',
      title: "Large text on the artwork, e.g. the version number.",
      eyebrow: "Small text above the title, e.g. “What's new in Kaeso”.",
      shape: "The artwork's aspect ratio.",
    },
  },
} satisfies ComponentDocs;
