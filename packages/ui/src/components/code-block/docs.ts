import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  CodeBlock: {
    description:
      "Highlighted code on a well: Kaeso's monochrome theme, copy in the header with a title or tabs and on hover without, tabs, line numbers, collapse with a fade at the cut. Long lines scroll sideways inside 16px of padding at both ends, and the code fades out at an edge with more to scroll to.",
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
      wrap: "Long lines wrap instead of scrolling sideways: code in a narrow column (a model's response). `phone` wraps them only below 640px, with wrapped parts indented, and scrolls from there up.",
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
      wrap: "Long lines wrap instead of scrolling sideways: code in a narrow column (a model's response). `phone` wraps them only below 640px, with wrapped parts indented, and scrolls from there up.",
    },
  },
  DiffView: {
    description:
      "Two versions of a text side by side in one column: removed lines on red, added lines on green, the changed words inside them marked, syntax highlighted, and long unchanged runs folded away.",
    props: {
      after: "The new version.",
      before: "The old version.",
      context: "Unchanged lines kept around each change before the rest folds (3 by default).",
      copy: "Hide the copy circle, which copies the diff with + and - markers.",
      lang: "The language of both versions, for highlighting (`text` by default).",
      lineNumbers: "Shows the old and the new line numbers.",
      surface: "On a filled well (`well`) or on the panel itself (`plain`).",
      title: "A name in the title bar, beside the count of added and removed lines.",
      wrap: "Long lines wrap instead of scrolling sideways, keeping their indent: values in a narrow column such as a sheet.",
    },
  },
  JsonView: {
    description:
      "A folding JSON tree: copy any path, highlight a search. For request and response bodies. It is one tab stop: the arrow keys move between rows and open and close them, and ⌘C or Ctrl C copies the row's path.",
    props: {
      defaultDepth: "Levels open at first (2 by default).",
      query: "Highlights keys and values that contain this text.",
      surface: "On a filled well (`well`) or on the panel itself (`plain`).",
      value: "The data to show as formatted JSON.",
    },
  },
} satisfies ComponentDocs;
