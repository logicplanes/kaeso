import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  CommandList: {
    description:
      "A search field over a long list that stays fast with thousands of rows: fuzzy matches on every row's words, groups with sticky captions, the best matches first, arrows, Home, End and Page keys, Enter to choose. The base of every picker.",
    props: {
      counts: "Shows each group's count beside its caption; false leaves them out, for a site whose house style has no result counters.",
      "aria-label": "The list's accessible name.",
      autoFocus: "Focuses the search field when it appears, as in a popover or a dialog step.",
      defaultQuery: "The search text on first render.",
      escapeClears: "Escape first clears the search, and only an empty search lets Escape close what holds the list (true by default).",
      emptyText:
        'What the list says when nothing matches: "No models match". Defaults to "No results for “claude”", naming the query, in the provider\'s language.',
      footer: "A line under the list, such as key hints.",
      groups:
        "The rows, in groups; a group with a label gets a sticky caption with its mark and count, and a loading group keeps its caption, without a count, over a Loading… row while it has no rows yet.",
      loading: "Shows a quiet progress line at the top while more rows load; with reduced motion it breathes instead of moving.",
      height:
        "How tall it is: bounded in a popover, a fixed raised frame in a dialog, or filling a phone sheet. Inside `PickerPopover` it follows the popover.",
      inputProps: "Data attributes for the search field, such as a marker other code looks for.",
      match: "Scores a row against the search: above 0 keeps it, and higher scores come first. Fuzzy matching on the label and keywords by default.",
      multiple: "Rows toggle with a checkbox instead of choosing one.",
      onEscape: "Called on Escape when the search field is already empty.",
      onQueryChange: "Called with the search text.",
      onInputKeyDown: "Called first with every key pressed in the search field; call `preventDefault` to replace the list's own handling.",
      onSelect: "Called with the value of the row chosen with Enter or a click.",
      placeholder: 'The search field\'s placeholder and accessible name ("Search" by default).',
      query: "The search text, when you control it.",
      searchPrefix: "Something before the search text, such as the page a palette is on.",
      size: "The search field's and the rows' height: 34px rows, or 40px in the command palette.",
      toolbar: "Controls under the search field: filter chips or a toggle.",
      value: "The chosen value, or the chosen values with `multiple`.",
    },
  },
  PickerPopover: {
    description: "The frame a picker opens in: a popover under its trigger on wider screens, a full-screen sheet with the search on top on phones.",
    props: {
      align: "Which edge of the trigger the popover lines up with.",
      children: "The `CommandList`.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      title: "The popover's accessible name and the sheet's title on phones.",
      trigger: "The button that opens it, usually a `PickerTrigger`.",
      width: "The popover's width on wider screens.",
    },
  },
  PickerTrigger: {
    description: "The pill that opens a picker and shows its value, in the same looks and sizes as `Select`.",
    props: {
      appearance: "A raised pill, quiet in a settings row, or transparent in a column header.",
      "aria-label":
        "What the picker is for. The pill is named by what it shows, so this becomes its description; it names the pill only while the pill shows nothing.",
      children: "The chosen value as it should read; the placeholder shows without it.",
      fullWidth: "Fills the container.",
      invalid: "Shows it as invalid; pair it with a message that says what to fix.",
      placeholder: "Text shown while nothing is chosen.",
      size: "The height of the pill.",
    },
  },
} satisfies ComponentDocs;
