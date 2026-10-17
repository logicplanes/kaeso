export interface SitePolicyOptions {
  hashes?: boolean;
  styleAttributes?: boolean;
}

export interface InlineContent {
  scripts: string[];
  styles: string[];
  styleAttributes: string[];
  handlers: string[];
  javascriptUrls: string[];
}

export interface SitePolicy {
  header: string;
  meta: string;
  problems: string[];
  files: string[];
  scripts: number;
  styles: number;
}

export const MAX_HEADER_BYTES: number;
export function inlineContent(html: string): InlineContent;
export function hashOf(text: string): string;
export function sitePolicy(dir: string, options?: SitePolicyOptions): SitePolicy;
export function withPolicyMeta(html: string, policy: string): { html: string; problem?: undefined } | { problem: string; html?: undefined };
export function writeSitePolicy(dir: string, options?: SitePolicyOptions): string;
