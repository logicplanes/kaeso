declare module "virtual:kaeso-docs/site/*" {
  import type { NavGroup, PageContent, PageMeta } from "~/markdown/ast";
  export const editBase: string;
  export const nav: NavGroup[];
  export const pages: Record<string, PageMeta>;
  export const load: Record<string, () => Promise<{ default: PageContent }>>;
}

declare module "virtual:kaeso-docs/search/*" {
  import type { SearchEntry } from "~/markdown/ast";
  const index: SearchEntry[];
  export default index;
}
