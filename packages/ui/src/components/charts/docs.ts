import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  BarChart: {
    description:
      "Bars for comparing values across categories: grouped, stacked, or horizontal for rankings. The category under the pointer, or reached with the arrow keys, shows its tooltip; the legend pills show and hide series.",
    props: {
      "aria-label": 'The chart\'s accessible name ("Requests by model").',
      axisLabels:
        'Shorter names under the bars, one per category ("3 Aug" for "Week of 3 Aug"); the tooltip, the table and screen readers keep the categories. Name the year where one begins ("Aug", "Dec", "2026", "Feb"), and pass `dates`: where the labels don\'t all fit, the axis keeps every nth, and keeps that one before the newest (`axisLabelIndexes`).',
      categories: "The names along the category axis, one per bar group, in order. They also head the tooltip and the data table.",
      dates:
        'The day each category begins, one per category: a `Date`, or an ISO day ("2026-01-05"). Where the axis thins its labels, it keeps the ones where a year begins (the month after December, the first week in January), read from these, so every language keeps the same labels.',
      empty:
        "Shows the empty state (`emptyText`) in place of the data, over the baseline and the category axis. By default it shows when there is no value at all; zeros are data.",
      emptyText: 'What the empty state says. Defaults to "No data for this period" in the provider\'s language.',
      error:
        "The data failed to load: the chart keeps its height, hides its legend and table toggle, and says so quietly, with Try again when there is `onRetry` and otherwise a hint to reload the page. Errors are toasts, so the value isn't shown; pass `true` and toast the message where the request fails.",
      formatValue:
        "Formats values in the tooltip, the table and beside horizontal bars. Without it, values are written in full and the axis in short form (1.2M); the axis takes the room its widest label needs, and long category names in a ranking end in an ellipsis with the full name on hover.",
      height: "The plot's height in px (240 by default).",
      highlight: "Highlight a category as the pointer does, with its tooltip: a linked view, or the states pages.",
      layout: "`grouped` puts each category's bars side by side (default), `stacked` stacks them, and `horizontal` draws bars across for rankings.",
      loading:
        "The data is loading: the chart keeps its height and is marked busy; data already on screen dims and breathes, and with none yet the plot is a skeleton.",
      onRetry: "Called when Try again is pressed after `error`; without it, the button doesn't show.",
      partial:
        'Marks the `first` or `last` category as covering only part of its span: its bars are drawn faint with a dashed edge, like a partial line, and its tooltip says "partial". `ongoing` marks the last one as still counting, such as today, and says "so far".',
      series:
        "The bars' series, each with an `id`, a `label`, one value per category and an optional `color`. Colors otherwise come from the series order by `id`; one series takes series 1. Negative values grow down from the zero line. With two or more, the legend shows and hides them.",
      shares:
        "Shows each series' share of the category's total in the tooltip (on by default). Turn it off when one series is part of another, such as errors within requests.",
    },
  },
  ChartFrame: {
    description:
      'The frame every chart draws in: a hidden data table for assistive technology, a "View as table" toggle that shows the table at the chart\'s height, scrolling inside, and the empty, loading and error states. The toggle sits at the right end of the last line of the legend; when the frame is narrower than 480px (the `chart-legend` container size, a phone), it moves to its own line under the legend, so it never reads as one of the series. Use it to build a chart of your own that behaves like the others.',
    props: {
      children: "The plot, usually an SVG, drawn at `height`.",
      height: "The plot's height in px. The empty, loading and error states keep it, so nothing below moves.",
      label: "The chart's accessible name, also the caption of the hidden data table.",
      legend: "A legend under the plot, usually a `Legend`. It is hidden while the table is shown.",
      state: "The chart's `loading`, `error`, `onRetry`, `empty` and `emptyText`, as the charts take them.",
      table: "The data as `columns` and `rows`, for screen readers and the table view. Without it there is no table and no toggle.",
      toggle: 'Shows the "View as table" button when there is a `table` (on by default).',
    },
  },
  ChartLive: {
    description:
      "A polite live region that announces what the keyboard highlight shows once it settles. Charts use it because their tooltip is hidden from assistive technology.",
    props: {
      children: "The line to announce, usually from `liveLine`; nothing while nothing is highlighted.",
    },
  },
  ChartTooltip: {
    description:
      "The tooltip a chart shows for a highlighted point or category: a heading with the x value and one row per series. It flips to the left past the middle and stays inside the chart.",
    props: {
      heading: "The heading, usually the highlighted time or category.",
      rows: "One row per series, each with an `id`, a `label`, a `color`, a `value` and an optional `share`.",
      visible: "Shows the tooltip; it fades out when false.",
      width: "The chart's width in px, so the tooltip flips and stays inside it.",
      x: "The anchor's x position in px from the chart's left edge. The tooltip sits 16px to its right.",
      xLeft: "Where the tooltip ends when it flips to the left, such as the left edge of a highlighted bar group, so it never covers it. Defaults to `x`.",
      y: "The anchor's y position in px from the chart's top.",
    },
  },
  Donut: {
    description: "A ring of shares with the total in the center and a legend beside it, or under it when narrow. Use it for a few parts of one whole.",
    props: {
      "aria-label": 'The chart\'s accessible name ("Requests by model").',
      empty:
        "Shows the empty state (`emptyText`) in place of the ring and legend. By default it shows when every value is 0, since a ring of nothing has no shares.",
      emptyText: 'What the empty state says. Defaults to "No data for this period" in the provider\'s language.',
      error:
        "The data failed to load: the chart keeps its height, hides its legend and table toggle, and says so quietly, with Try again when there is `onRetry` and otherwise a hint to reload the page. Errors are toasts, so the value isn't shown; pass `true` and toast the message where the request fails.",
      formatValue: "Formats the total in the center and the values in the legend. Defaults to `formatNumber`.",
      loading:
        "The data is loading: the chart keeps its height and is marked busy; data already on screen dims and breathes, and with none yet the plot is a skeleton.",
      onRetry: "Called when Try again is pressed after `error`; without it, the button doesn't show.",
      size: "The ring's diameter: `sm` 140px, `md` 160px (default) or `lg` 180px.",
      slices:
        "The parts of the whole, each with an `id`, a `label`, a `value` and an optional `color`. Slices under 2% are grouped as Other when there are two or more, and past the eighth slice the rest are grouped too, so no two slices share a color.",
      total:
        "Replaces the total in the center, which is otherwise the sum of the slices written with `formatValue`. A long total steps down a size to fit the ring.",
      totalLabel: 'The word under the total. Defaults to "Total" in the provider\'s language.',
    },
  },
  Gauge: {
    description:
      'A semicircle meter for one share of a limit, such as "budget used". At the first threshold it says "Near the limit" and turns to warning; at 100% it says "Over the limit" and turns to danger.',
    props: {
      "aria-label": 'The chart\'s accessible name ("Requests by model").',
      label: 'A caption under the arc ("of $500.00 budget"); it wraps when long.',
      thresholds: "Fractions marked as ticks on the arc (`[0.8]` by default). From the lowest one, the arc turns to warning.",
      value: "The share used, from 0 to 1. Values above 1 fill the arc and turn it to danger; a missing value (NaN) shows a dash and no arc.",
      valueLabel: "Replaces the value in the center, which is otherwise the percentage. Pass a string so screen readers hear it with the unit.",
    },
  },
  Heatmap: {
    description:
      "A grid of cells shaded on a five-step ramp, for one value across two dimensions such as hour by weekday. One tab stop: the arrow keys move between cells and show each value, the table toggle lists them all, and a grid wider than its column scrolls with a fade at the edge.",
    props: {
      "aria-label": 'The chart\'s accessible name ("Requests by model").',
      columnLabelEvery: "Labels every nth column, starting with the first (3 by default), so dense columns stay readable.",
      columns: "The column labels, left to right (hours, or weeks).",
      formatValue: "Formats a cell's value for its tooltip, the table and screen readers. Defaults to `formatNumber`.",
      rows: "The row labels, top to bottom (weekdays). With no rows or columns the chart shows its empty state.",
      values: "The values by row, then column: `values[row][column]`. The darkest step is the largest value.",
    },
  },
  Legend: {
    description: "Pills with swatches that name a chart's series. With `onToggle` they show and hide series, and hovering one can isolate it.",
    props: {
      hidden: "The `id`s of the series that are hidden; their pills show as off.",
      items: "One item per series, each with an `id`, a `label`, a `color` and an optional `value`.",
      onHover: "Called with a series `id` when its pill is hovered or focused, and with `null` when it is left.",
      onToggle: "Called with a series `id` when its pill is pressed. Without it, the pills are plain labels.",
      shape: "The swatch: `square` for filled marks (default) or `line` for lines.",
    },
  },
  Ring: {
    description: "The same component as `Donut` under another name, with the same props.",
    props: {
      "aria-label": 'The chart\'s accessible name ("Requests by model").',
      empty:
        "Shows the empty state (`emptyText`) in place of the ring and legend. By default it shows when every value is 0, since a ring of nothing has no shares.",
      emptyText: 'What the empty state says. Defaults to "No data for this period" in the provider\'s language.',
      error:
        "The data failed to load: the chart keeps its height, hides its legend and table toggle, and says so quietly, with Try again when there is `onRetry` and otherwise a hint to reload the page. Errors are toasts, so the value isn't shown; pass `true` and toast the message where the request fails.",
      formatValue: "Formats the total in the center and the values in the legend. Defaults to `formatNumber`.",
      loading:
        "The data is loading: the chart keeps its height and is marked busy; data already on screen dims and breathes, and with none yet the plot is a skeleton.",
      onRetry: "Called when Try again is pressed after `error`; without it, the button doesn't show.",
      size: "The ring's diameter: `sm` 140px, `md` 160px (default) or `lg` 180px.",
      slices:
        "The parts of the whole, each with an `id`, a `label`, a `value` and an optional `color`. Slices under 2% are grouped as Other when there are two or more, and past the eighth slice the rest are grouped too, so no two slices share a color.",
      total:
        "Replaces the total in the center, which is otherwise the sum of the slices written with `formatValue`. A long total steps down a size to fit the ring.",
      totalLabel: 'The word under the total. Defaults to "Total" in the provider\'s language.',
    },
  },
  Sankey: {
    description:
      "A flow diagram, such as requests from keys to models to providers, with ribbons as wide as each flow. Hovering a node, or reaching it with the arrow keys, highlights its paths. Each column keeps room for its labels; when narrow, the values go first, then long names end in an ellipsis.",
    props: {
      "aria-label": 'The chart\'s accessible name ("Requests by model").',
      empty: "Shows the empty state (`emptyText`) in place of the data. By default it shows when no link carries a value.",
      emptyText: 'What the empty state says. Defaults to "No data for this period" in the provider\'s language.',
      error:
        "The data failed to load: the chart keeps its height, hides its legend and table toggle, and says so quietly, with Try again when there is `onRetry` and otherwise a hint to reload the page. Errors are toasts, so the value isn't shown; pass `true` and toast the message where the request fails.",
      formatValue: "Formats the values beside the nodes and in the data table. Defaults to `formatNumber`.",
      height: "The diagram's height in px (320 by default).",
      links: "The flows, each with a `source` and a `target` node `id` and a `value`.",
      loading:
        "The data is loading: the chart keeps its height and is marked busy; data already on screen dims and breathes, and with none yet the plot is a skeleton.",
      nodes: "The nodes, each with an `id` and a `label`. Columns follow from the links.",
      onRetry: "Called when Try again is pressed after `error`; without it, the button doesn't show.",
    },
  },
  ShareBar: {
    description: "One pill split into segments by share, with a legend under it. Use it for a compact breakdown of one whole in a row or a card.",
    props: {
      "aria-label": 'The chart\'s accessible name ("Requests by model").',
      formatValue: "Formats each segment's value for screen readers, beside its share. Defaults to `formatNumber`.",
      legend: "Shows the legend with each share under the bar (on by default).",
      slices: "The parts of the whole, each with an `id`, a `label`, a `value` and an optional `color`.",
    },
  },
  Sparkline: {
    description: "A small line chart of one series, inline beside a value, with its last point marked. Set `fluid` to fill a stat tile's width.",
    props: {
      "aria-label": 'The chart\'s accessible name ("Requests by model").',
      color: "The line's color (`var(--k-viz-1)` by default). Pass `seriesColor(id)` to match the series elsewhere.",
      fluid: "Fills the container's width at 32px high, without the last point's dot, as in a stat tile. Otherwise it is 80×24.",
      values: "The values in order, oldest first. One value is a dot at the newest end; none is a quiet flat line.",
    },
  },
  Swatch: {
    description: "The mark that keys a series: a short stroke for lines, a small rounded square for filled marks. Never a dot.",
    props: {
      color: "The series color.",
      off: "Draws the swatch as an outline in the series color, for a hidden series.",
      shape: "`square` for filled marks (default) or `line` for lines.",
    },
  },
  TimeseriesChart: {
    description:
      "A line or area chart of values over time, such as usage, with a crosshair and tooltip. The point under the pointer, or reached with the arrow keys, shows its values.",
    props: {
      "aria-label": 'The chart\'s accessible name ("Requests by model").',
      empty:
        "Shows the empty state (`emptyText`) in place of the data, over the baseline and the time axis. By default it shows when there are no points; zeros are data and draw a flat line on the baseline.",
      emptyText: 'What the empty state says. Defaults to "No data for this period" in the provider\'s language.',
      error:
        "The data failed to load: the chart keeps its height, hides its legend and table toggle, and says so quietly, with Try again when there is `onRetry` and otherwise a hint to reload the page. Errors are toasts, so the value isn't shown; pass `true` and toast the message where the request fails.",
      formatTick: 'Names the times under the axis ("3 Aug", "2026"). Defaults to calendar ticks in `timeZone`; pass it with `tickAt="points"` for buckets.',
      formatTime:
        'Names a point\'s time in the tooltip, the table and for screen readers, where a point stands for more than its instant: "Week of 3 Aug", "Aug 2026".',
      formatValue:
        "Formats values in the tooltip, the table and on the axis. Without it, values are written in full and the axis in short form (1.2M), and the axis takes the room its widest label needs.",
      height: "The plot's height in px (240 by default).",
      hideLegend: "Hide the legend (one series).",
      highlight: "Highlight a point in time as the pointer does, with the crosshair and tooltip: a linked view, or the states pages.",
      kind: "`line` draws lines (default), `area` adds a light fill under each line, and `stacked` stacks the areas.",
      loading:
        "The data is loading: the chart keeps its height and is marked busy; data already on screen dims and breathes, and with none yet the plot is a skeleton.",
      onRetry: "Called when Try again is pressed after `error`; without it, the button doesn't show.",
      partial:
        'Marks the `first` or `last` point as covering only part of its span: the line to it is dashed and its tooltip says "partial". `ongoing` marks the last one as still counting, such as today, and says "so far".',
      series: "The series, each with an `id`, a `label`, `points` of `t` (a time) and `v`, and an optional `color`.",
      shares:
        "Shows each series' share of the point's total in the tooltip (on by default). Turn it off when one series is part of another, such as errors within requests.",
      threshold: "A threshold line (a budget) in danger, dashed.",
      tickAt:
        "`time` labels round times, as many as fit (default). `points` sets each point in the middle of an equal step and labels every nth one, as a bar chart places and labels its categories, so a bar chart of the same days lines up with it.",
      timeZone:
        'The zone the axis and the default labels read times in, such as "UTC" for days counted in UTC. Defaults to the zone set with `setFormatTimeZone`, else the viewer\'s.',
    },
  },
} satisfies ComponentDocs;
