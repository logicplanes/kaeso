export type CodeLang = "ts" | "tsx" | "js" | "json" | "bash" | "python" | "yaml" | "diff" | "text";

export type CalloutKind = "note" | "tip" | "important" | "warning" | "caution";

export type Inline =
  | { t: "text"; v: string }
  | { t: "strong" | "em" | "del"; c: Inline[] }
  | { t: "code"; v: string }
  | { t: "br" }
  | { t: "link"; kind: "page" | "anchor" | "external" | "file"; href: string; title?: string; c: Inline[] }
  | { t: "img"; src: string; alt: string; title?: string; width?: number; height?: number };

export interface ListItem {
  task?: boolean;
  checked?: boolean;
  c: Block[];
}

export type Block =
  | { t: "heading"; level: 2 | 3 | 4 | 5 | 6; id: string; c: Inline[] }
  | { t: "paragraph"; c: Inline[]; tight?: boolean }
  | { t: "list"; ordered: boolean; start: number; items: ListItem[] }
  | { t: "code"; lang: CodeLang; code: string; title?: string }
  | { t: "quote"; c: Block[] }
  | { t: "callout"; kind: CalloutKind; c: Block[] }
  | { t: "table"; align: ("start" | "end")[]; head: Inline[][]; rows: Inline[][][] }
  | { t: "rule" };

export interface TocEntry {
  id: string;
  label: string;
  depth: 0 | 1;
}

export type ChangelogGroup = "added" | "changed" | "deprecated" | "removed" | "fixed" | "security";

export interface ChangelogTopic {
  name: string;
  id: string;
  blocks: Block[];
}

export interface ChangelogSection {
  name: string;
  group?: ChangelogGroup;
  id: string;
  blocks: Block[];
  topics: ChangelogTopic[];
}

export interface ChangelogEntry {
  id: string;
  intro: Block[];
  sections: ChangelogSection[];
}

export interface ChangelogRelease extends ChangelogEntry {
  version: string;
  date?: string;
  prerelease: boolean;
  yanked: boolean;
  latest: boolean;
  previous?: string;
  url?: string;
  compare?: string;
}

export interface ChangelogContent {
  releases: ChangelogRelease[];
  unreleased?: ChangelogEntry & { since?: string; compare?: string };
  repository?: string;
}

export interface PageContent {
  blocks: Block[];
  toc: TocEntry[];
  changelog?: ChangelogContent;
}

export interface PageMeta {
  path: string;
  title: string;
  description: string;
  label: string;
  file: string;
  prev?: { path: string; title: string };
  next?: { path: string; title: string };
}

export interface NavItem {
  path: string;
  label: string;
  depth: 0 | 1 | 2;
}

export interface NavGroup {
  id: string;
  label?: string;
  items: NavItem[];
}

export interface SearchEntry {
  path: string;
  title: string;
  section?: string;
  description: string;
  headings: { id: string; label: string }[];
  words: string;
}
