import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Thumbnail: {
    description:
      "A square image someone sent, such as a photo in a conversation: cropped to fill, on a well with radius 14; when it can't load, its text alternative shows instead.",
    props: {
      alt: "What the image shows, read by screen readers and shown in its place when it can't load.",
      size: "The width and height: 64, 120 or 160 pixels.",
      src: "The image's address, including a data: URL.",
    },
  },
} satisfies ComponentDocs;
