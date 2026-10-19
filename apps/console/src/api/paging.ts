export interface Page<T> {
  data: T[];
  has_more: boolean;
  next_cursor?: string | null | undefined;
}

export function nextPageParam<T>(page: Page<T>): string | undefined {
  return page.has_more && page.next_cursor ? page.next_cursor : undefined;
}

export const refetchOnFocusWhileOnePage = (query: { state: { data: { pages: readonly unknown[] } | undefined } }): boolean =>
  (query.state.data?.pages.length ?? 0) <= 1;

export async function allPages<T>(fetchPage: (after: string | undefined) => Promise<Page<T>>): Promise<T[]> {
  const out: T[] = [];
  let after: string | undefined;
  do {
    const page = await fetchPage(after);
    out.push(...page.data);
    after = nextPageParam(page);
  } while (after);
  return out;
}
