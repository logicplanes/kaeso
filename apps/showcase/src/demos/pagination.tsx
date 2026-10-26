import { useState } from "react";
import { Pagination } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Pagination",
  description: "Cursor-style paging for the console's APIs: “Load more”, or previous and next.",
  group: "Components",
  entry: "pagination",
  import: "Pagination",
};
export const examples = [
  { id: "load-more", title: "Load more", fn: "LoadMore" },
  { id: "cursor", title: "Previous and next", fn: "Cursor" },
  { id: "pages", title: "Page numbers", fn: "Pages" },
];
export function LoadMore() {
  const [n, setN] = useState(50);
  return <Pagination hasMore={n < 200} onLoadMore={() => setN((x) => Math.min(200, x + 50))} />;
}
export function Cursor() {
  return <Pagination variant="cursor" hasMore hasPrevious={false} />;
}
export function Pages() {
  const [p, setP] = useState(4);
  return <Pagination variant="pages" page={p} pageCount={12} onPageChange={setP} />;
}
export function States() {
  return (
    <>
      <StateSection title="Load more">
        <div className="flex flex-col items-start gap-8">
          <Pagination hasMore />
          <Pagination hasMore loading />
        </div>
      </StateSection>
      <StateSection title="Previous and next: both, previous only, next only">
        <div className="flex flex-col items-start gap-16">
          <Pagination variant="cursor" hasMore hasPrevious />
          <Pagination variant="cursor" hasPrevious />
          <Pagination variant="cursor" hasMore />
        </div>
      </StateSection>
      <StateSection title="Page numbers: seven slots at the start, middle and end, so Next never moves">
        <div className="flex flex-col items-start gap-16">
          <Pagination variant="pages" page={1} pageCount={12} />
          <Pagination variant="pages" page={6} pageCount={12} />
          <Pagination variant="pages" page={12} pageCount={12} />
          <Pagination variant="pages" page={57} pageCount={120} />
          <Pagination variant="pages" page={1} pageCount={1} />
        </div>
      </StateSection>
    </>
  );
}
