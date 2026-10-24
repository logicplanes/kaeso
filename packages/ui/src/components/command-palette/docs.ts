import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  CommandPalette: {
    description: "⌘K: search every page and action; nested pages; async groups with a quiet progress line.",
    props: {
      counts: "Shows each group's count beside its caption; false leaves them out, for a site whose house style has no result counters.",
      defaultQuery: "Start with this query (documentation).",
      emptyText: 'Defaults to "No results" in the provider\'s language.',
      groups: "The commands, in groups with a heading each.",
      hotkey: "Open with ⌘K / Ctrl+K anywhere (default true).",
      inline: "Render without a portal, for documentation and state matrices.",
      keyHints: "A footer with the keys (↑ ↓ to navigate, ↵ to open, esc to close). Off by default: key hints only where asked for.",
      onOpenChange: "Called when it opens or closes.",
      onQueryChange: "Called as the query changes, for async groups.",
      open: "Whether it is open, when you control it.",
      placeholder: 'Defaults to "Search or run a command" in the provider\'s language.',
      query: "The query in the field, when you control it; keep it in sync through onQueryChange.",
    },
  },
} satisfies ComponentDocs;
