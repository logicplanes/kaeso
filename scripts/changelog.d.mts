export interface ChangelogTopic {
  name: string;
  id: string;
  markdown: string;
  line: number;
}

export interface ChangelogSection {
  name: string;
  id: string;
  markdown: string;
  line: number;
  topics: ChangelogTopic[];
}

export interface ChangelogEntry {
  id: string;
  intro: string;
  introLine: number;
  sections: ChangelogSection[];
}

export interface ChangelogUnreleased extends ChangelogEntry {
  since: string | null;
  compare: string | null;
}

export interface ChangelogRelease extends ChangelogEntry {
  version: string;
  date: string | null;
  prerelease: boolean;
  yanked: boolean;
  latest: boolean;
  previous: string | null;
  url: string | null;
  compare: string | null;
}

export interface ChangelogHistory {
  repository: string | null;
  definitions: string;
  definitionsLine: number | null;
  unreleased: ChangelogUnreleased | null;
  releases: ChangelogRelease[];
}

export interface PullRequest {
  number: number;
  title: string;
  url: string;
  labels: string[];
  author?: string;
}

export function history(text: string): ChangelogHistory;
export function generate(pulls: PullRequest[]): string[];
export function releasable(text: string, version: string, pulls?: PullRequest[]): boolean;
export function draft(text: string, version: string, pulls?: PullRequest[]): string[];
export function release(text: string, version: string, date: string, pulls?: PullRequest[], written?: string[] | null): string;
export function notes(text: string, version: string): string;
