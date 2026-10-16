import type { ComponentDocs } from "../../component-docs";

export const docs = {
  FilterRow: {
    description:
      "The row between a page's header and its list or table: pill tabs at the start, filters at the end. It grows with its content instead of keeping a toolbar's 48px, and on a narrow screen the filters wrap under the tabs, at the start. The tabs keep their own arrow keys; Left and Right move across the filters only, which form their own toolbar.",
    props: {
      children: "Filters at the end of the row: a `Select`, a `SearchInput`. At the start when there are no tabs.",
      filtersLabel: "The name of the filters' toolbar. Defaults to \"Filters\" in the provider's language.",
      size: "By default `inset` in a settings column and `md` elsewhere.",
      tabs: "Pill tabs at the start of the row (`PillTabs`). In a settings column their first label lines up with the title.",
    },
  },
} satisfies ComponentDocs;
