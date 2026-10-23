import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  CodeBlock: {
    description:
      "Highlighted code on a well: Kaeso's monochrome theme, copy on hover, tabs, line numbers, collapse. Long lines scroll sideways inside 16px of padding at both ends, and the code fades out at an edge with more to scroll to.",
    props: {
      code: "The code to show.",
      collapseAfter: "Collapse beyond this many lines (24 by default; 0 never collapses).",
      copy: "Hide the copy circle.",
      highlightLines: "1-based line numbers to highlight.",
      lang: "The language of the code, for highlighting.",
      lineNumbers: "Shows line numbers.",
      surface: "On a filled well (`well`) or on the panel itself (`plain`).",
      onTabChange: "Called with the label of the tab a person chooses, to remember it.",
      tab: "The label of the tab to show, such as a language the reader chose before. A label the block lacks leaves its own choice.",
      tabs: 'Several versions: "Python · Node · curl", shown as pill tabs.',
      title: "A file name in the title bar.",
      wrap: "Long lines wrap instead of scrolling sideways: code in a narrow column (a model's response).",
    },
  },
  CodeHighlighted: {
    description: "The same component as `CodeBlock`, under another name.",
    props: {
      code: "The code to show.",
      collapseAfter: "Collapse beyond this many lines (24 by default; 0 never collapses).",
      copy: "Hide the copy circle.",
      highlightLines: "1-based line numbers to highlight.",
      lang: "The language of the code, for highlighting.",
      lineNumbers: "Shows line numbers.",
      surface: "On a filled well (`well`) or on the panel itself (`plain`).",
      onTabChange: "Called with the label of the tab a person chooses, to remember it.",
      tab: "The label of the tab to show, such as a language the reader chose before. A label the block lacks leaves its own choice.",
      tabs: 'Several versions: "Python · Node · curl", shown as pill tabs.',
      title: "A file name in the title bar.",
      wrap: "Long lines wrap instead of scrolling sideways: code in a narrow column (a model's response).",
    },
  },
  JsonView: {
    description: "A folding JSON tree: copy any path, highlight a search. For request and response bodies.",
    props: {
      defaultDepth: "Levels open at first (2 by default).",
      query: "Highlights keys and values that contain this text.",
      surface: "On a filled well (`well`) or on the panel itself (`plain`).",
      value: "The data to show as formatted JSON.",
    },
  },
} satisfies ComponentDocs;
