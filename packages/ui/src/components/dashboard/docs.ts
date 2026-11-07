import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Dashboard: {
    description:
      "An overview page's numbers, charts and lists on one raised surface split by hairlines, the way a `StatGroup` splits its tiles. As many columns from 1024px as `columns` says (twelve by default), one below it; each `DashboardCell` takes its span through `className` (`lg:col-span-2`). Put `MetricTabs` first, so the totals lead and pick what the chart beneath them shows.",
    props: {
      "aria-label": 'The surface\'s accessible name, such as "Usage in the period".',
      busy: "Keeps the numbers on screen, a step back, while the next ones load.",
      headingLevel: "The level of the cells' titles: 3 below a section's heading (the default), 2 when the page's title is the only heading above.",
      children: "`MetricTabs` and `DashboardCell`s.",
      wideColumns:
        "Columns from 1280px, when the surface needs more room per cell than `columns` gives at 1024px: two at 1024px and four at 1280px keeps every cell wide enough to read.",
      columns:
        "Columns from 1024px. Give it as many as `MetricTabs` has tabs, and let every cell span whole tabs, so each hairline below continues a tab's edge; 12 for free layouts.",
    },
  },
  DashboardCell: {
    description:
      "One cell of a `Dashboard`: a quiet label, at most one or two ghost buttons beside it, and its content. Text and numbers sit on the cell's text edge through `Readout` and `CellInset`; `RankList` rows run from the cell's own edge so their hover fills line up with the label.",
    props: {
      actions: 'Ghost buttons that open the page behind the cell: "Request logs", "Limits".',
      children: "The cell's content: a `Readout`, then a chart, a meter or a `RankList`.",
      href: "Turns the title into a link with a chevron, to the page that tells the cell's whole story.",
      title: 'What the cell shows: "Cache hit", "Models by requests".',
    },
  },
  CellInset: {
    description:
      "Places a chart, a meter or a note on the cell's text edge, 12px in from where rows start, so it lines up with the label and the number above.",
    props: {
      children: "What sits on the text edge.",
      grow: "Takes the cell's remaining height, so a chart or a note can sit at the bottom with `mt-auto`.",
    },
  },
  MetricTabs: {
    description:
      "The totals of a period as tabs across the top of a `Dashboard`: each says its label, its number and its change. The chosen one has an ink bar along its foot and decides what the chart below draws; arrow keys, Home and End move between them.",
    props: {
      "aria-label": 'The tabs\' accessible name, such as "Metric to chart".',
      controls: "The id of the chart the tabs drive.",
      items: 'One tab per total: `id`, `label`, the formatted `value`, an optional `change` and a `detail` such as "vs. prior 30 days".',
      loading: "While the totals load: every tab keeps its height and shows the number's shape.",
      orientation:
        "Across the top of the surface (`horizontal`), or in a side column (`vertical`) from 1280px and a two-by-two grid below it, each tab with its number on its own line and its trend underneath.",
      onValueChange: "Called with the chosen tab's id.",
      value: "The chosen tab's id.",
    },
  },
  Readout: {
    description:
      "A cell's number on its text edge: the value and an optional unit word, then a line with its change or a quiet note. The line is always there at `md`, so readouts side by side start their content at the same height.",
    props: {
      change: "The change against the previous period, written as text by `Delta`.",
      detail: 'A quiet note after the change: "0.7% of requests".',
      label: "A small label above a `sm` readout, when two sit side by side.",
      loading: "While the number loads: the readout keeps its height.",
      size: "The main number of a cell (`md`), or a smaller one beside another (`sm`).",
      unit: 'The word after the number, on its baseline: "tokens".',
      value: 'The number, already formatted: "$60.45", "48%".',
    },
  },
  RankList: {
    description:
      "The top rows of a breakdown, largest first: the label, the share and the number in columns that line up from row to row, with each row's share of the largest drawn behind it in a neutral fill. Rows open something when `onOpen` is set, such as the request logs filtered to that model.",
    props: {
      "aria-label": "The list's accessible name.",
      empty: 'What an empty list says: "Nothing in this period".',
      items: "The rows: `id`, `label`, `value`, `secondary` (the share or a time) and `share` for the bar; leave `share` out for plain rows.",
      loading: "While the rows load: `rows` lines of their shape.",
      onOpen: "Called with a row's id when it is chosen; without it, rows are plain text.",
      rows: "How many lines the loading state shows.",
    },
  },
  ModelCard: {
    description:
      "A model worth a look, as a cell of a `Dashboard`: its provider's mark, name and id, a line about it, a few facts such as context and price, and one action.",
    props: {
      action: 'One button: "Add to this project", "Open in models".',
      badge: 'A status beside the mark, such as "New".',
      detail: "One line about the model, such as when it was released.",
      facts: "Two to four label and value pairs: context, price per million tokens.",
      model: "The model's id, in mono.",
      name: "The model's name.",
      mark: "The provider's mark: a `ProviderBadge` at size `lg`.",
    },
  },
  Announcement: {
    description:
      "One piece of news across the top of a home page: an eyebrow, a title, a line and an action beside its artwork, such as a new release with a flush `ReleaseArt`. The artwork leaves on narrow screens.",
    props: {
      actions: "One or two buttons.",
      art: "The artwork on the right, 16:9 at 320px: a `ReleaseArt` with `flush`, so the whole cover shows.",
      description: "One line, two at most.",
      eyebrow: 'What kind of news: "New release".',
      title: "The news itself.",
    },
  },
  Legend: {
    description: "A chart's key that also says each series' total: a solid line for this period, a dashed one for the period it is compared with.",
    props: {
      items: "One entry per series: `label`, `value`, `color`, and `dashed` for the comparison.",
    },
  },
} satisfies ComponentDocs;
