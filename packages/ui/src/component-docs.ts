export interface ComponentDoc {
  readonly description: string;
  readonly props: Readonly<Record<string, string>>;
  readonly deprecated?: Readonly<Record<string, string>> | undefined;
}

export type ComponentDocs = Readonly<Record<string, ComponentDoc>>;

export const COMMON_PROP_DOCS: Readonly<Record<string, string>> = {
  className: "Classes for the layout of the outer element: margin, width and placement, never its look.",
  "data-force": "Forces an interaction state for state matrices: `hover`, `pressed`, `focus`, `open`.",
  style: "Inline styles for the outer element, for layout only.",
};
