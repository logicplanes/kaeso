import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Markdown: {
    description:
      'Markdown, safely: headings, paragraphs, lists, tasks, code fences (CodeBlock), inline code, emphasis, links, quotes, rules and tables. Raw HTML stays text, links open only for http, https and mailto in a new tab, images are links. Long texts show their start until "Show all".',
    props: {
      collapseAfter: 'Texts longer than this many characters show their start and "Show all" (24,000 by default; 0 never collapses).',
      headingLevel: "The heading level `#` becomes (3 by default: below a page's and a section's own headings).",
      size: "The text size: `small`, `body` or `lg`.",
      streaming: "The text is still arriving: nothing collapses, and the tokens aren't cached.",
      text: "The Markdown source. Raw HTML in it stays text.",
      tone: "Primary text (`default`) or secondary text (`secondary`).",
      trailing: "Drawn inline after the last word of a paragraph, heading or list item (never on a line of its own).",
      wrapCode: "Long code lines wrap instead of scrolling sideways (narrow columns).",
    },
  },
} satisfies ComponentDocs;
