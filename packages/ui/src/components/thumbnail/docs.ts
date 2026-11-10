import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  MediaView: {
    description:
      "An image, an audio clip, a video or a file someone sent or a model made, with a line that names it and a download: an image is a thumbnail that opens to the whole image in place, audio and video play in the browser's own player.",
    props: {
      detail: "What the file is, after its name: its format, dimensions and size.",
      download: "The address the download saves from, when it isn't the one shown or played, such as a blob: URL beside an image's data: URL.",
      filename: "The name the download saves under; without it the browser chooses one.",
      kind: "What the file is: an image, audio, a video, or a file that can only be downloaded.",
      label: "What the file is or shows, in words: the caption, the image's text alternative and the player's accessible name.",
      src: "The file's address: a data: URL for an image, a blob: URL for audio and video where the content security policy allows no other.",
    },
  },
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
