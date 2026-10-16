import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  AuthLayout: {
    description:
      "Sign-in and setup: no chrome, a centered 400px column on the panel with the title and description centered above the form. The wordmark sits top left exactly where the sidebar header puts it in every other layout: as far in as it is down (28px; centered in a 52px row 8px from the top).",
    props: {
      children: "The form.",
      description: "One sentence under the heading.",
      footer: "Content under the form, such as a link to sign up.",
      title: "The page's heading.",
    },
  },
  Section: {
    description: "A section of a page: a title-sm heading with optional description and actions, no frame.",
    props: {
      actions: "Actions at the end of the heading row.",
      children: "The section's content.",
      description: "One sentence under the heading.",
      spacing: "The space above the section: the default, or none.",
      title: "The section's heading.",
    },
  },
  SplitView: {
    description:
      "Main content and a 320px aside, separated by space and a subtle vertical line. On wide screens the aside stays in view while the main content scrolls.",
    props: {
      aside: "The side column; it moves under the content on phones.",
      children: "The main content.",
    },
  },
} satisfies ComponentDocs;
