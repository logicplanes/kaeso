import { useState } from "react";
import { JsonView, SearchInput } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

const body = {
  model: "claude-sonnet-4-5",
  max_tokens: 1024,
  stream: true,
  metadata: { user_id: "usr_ada", team: "checkout" },
  messages: [{ role: "user", content: "Can I still change the delivery address on order 4471?" }],
  tools: [{ name: "get_order", input_schema: { type: "object", properties: { order_id: { type: "string" } }, required: ["order_id"] } }],
  temperature: null,
};

export const meta = {
  title: "JsonView",
  description: "A folding JSON tree: copy any path, highlight a search.",
  group: "Components",
  entry: "code-block",
  import: "JsonView",
  keyboard: [
    ["Tab", "Moves into the tree, then out of it: the tree is one tab stop"],
    ["↑ ↓", "Moves between rows"],
    ["→ ←", "Opens and closes an object or array, or moves into it and back to its parent"],
    ["Enter Space", "Opens or closes an object or array"],
    ["⌘C Ctrl C", "Copies the row's path"],
  ],
  a11y: ["The tree is a tree of rows with their level and open state; a copied path is announced."],
};
export const examples = [{ id: "basic", title: "Request body", fn: "Basic" }];
export function Basic() {
  const [q, setQ] = useState("");
  return (
    <div className="flex w-560 flex-col gap-12">
      <SearchInput aria-label="Search JSON" placeholder="Search keys and values" value={q} onChange={(e) => setQ(e.target.value)} onClear={() => setQ("")} />
      <JsonView value={body} query={q || undefined} />
    </div>
  );
}
const deep = {
  request: {
    id: "req_01JAR2N7Q8",
    metadata: { team: "checkout", region: "eu-west-1", tags: { priority: "high", retries: { max: 3, backoff: "exponential" } } },
    prompt_cache_key: "checkout-assistant-system-prompt-version-2026-10-12-production",
  },
  empty: {},
};

export function States() {
  return (
    <>
      <StateSection title="Depths and search">
        <div className="flex w-560 flex-col gap-16">
          <JsonView value={body} defaultDepth={1} />
          <JsonView value={body} defaultDepth={3} query="order" />
        </div>
      </StateSection>
      <StateSection title="Deep nesting, long strings and an empty object at 320px">
        <div className="w-320">
          <JsonView value={deep} defaultDepth={5} />
        </div>
      </StateSection>
    </>
  );
}
