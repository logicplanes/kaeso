import demosJson from "~/generated/demos.json";
import docPagesJson from "~/generated/doc-pages.json";

export interface DemoExample {
  id: string;
  title: string;
  description?: string;
  fn: string;
  code: string;
  minHeight?: number;
}

export interface DemoSnippet {
  fn: string;
  code: string;
}

export interface DemoPage {
  slug: string;
  title: string;
  description: string;
  group: string;
  entry?: string;
  import?: string;
  barrel?: boolean;
  source?: string;
  keyboard?: string[][];
  a11y?: string[];
  examples: DemoExample[];
  dos: DemoSnippet[];
  donts: DemoSnippet[];
  hasStates: boolean;
}

export interface DocProp {
  name: string;
  type: string;
  default: string | null;
  description: string;
}

export interface DocPageData {
  importLine: string;
  props: DocProp[];
}

export const demos: Record<string, DemoPage> = demosJson;
export const docPages: Record<string, DocPageData> = docPagesJson;
