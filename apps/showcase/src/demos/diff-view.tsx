import { DiffView } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

const json = (v: unknown) => JSON.stringify(v, null, 2);
const credential = {
  account: "owner@example.com",
  auth_method: "grok",
  created_at: "2026-10-23T11:29:34Z",
  id: "cred_01m5m8c02wesfvakyq92k9wtgy",
  is_default: false,
  key_id: "k184dc59ed4",
  name: "Grok plan",
  projects: [{ avatar_url: null, id: "proj_01m5m8f0beeb8vqk4x6852hbcv", name: "Checkout" }],
  provider: "xai",
  updated_at: "2026-10-23T11:29:34Z",
};
const moved = { ...credential, projects: [{ avatar_url: null, id: "proj_01m5p292xvef2v2ss387kgrvt2", name: "Agents" }] };
const limit = { name: "Daily spend", period: "day", amount_usd: 50, alert_at: [0.5, 0.8] };
const raised = { name: "Daily spend", period: "day", amount_usd: 120, alert_at: [0.5, 0.8, 0.95] };

export const meta = {
  title: "DiffView",
  description: "Two versions in one column: removed lines on red, added on green, changed words marked, unchanged runs folded.",
  group: "Components",
  entry: "code-block",
  import: "DiffView",
  a11y: [
    "Each changed line starts with “Added” or “Removed” for screen readers; the + and − marks and the colour are only for sight.",
    "The title bar says how many lines were added and removed, and each folded run is a button that names how many lines it shows.",
  ],
};
export const examples = [
  { id: "audit", title: "An audit event", fn: "Audit" },
  { id: "numbers", title: "With line numbers", fn: "Numbers" },
];
export function Audit() {
  return (
    <div className="w-560 max-w-full">
      <DiffView title="Before → after" lang="json" before={json(credential)} after={json(moved)} wrap />
    </div>
  );
}
export function Numbers() {
  return (
    <div className="w-560 max-w-full">
      <DiffView title="limit.json" lang="json" before={json(limit)} after={json(raised)} lineNumbers />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Changes">
        <div className="flex w-560 max-w-full flex-col gap-24">
          <Audit />
          <Numbers />
          <DiffView title="Created" lang="json" before="" after={json(limit)} />
          <DiffView title="Unchanged" lang="json" before={json(limit)} after={json(limit)} />
        </div>
      </StateSection>
      <StateSection title="Plain text, no copy">
        <div className="w-560 max-w-full">
          <DiffView surface="plain" copy={false} before={"Answer in English.\nKeep it short."} after={"Answer in the user's language.\nKeep it short."} />
        </div>
      </StateSection>
    </>
  );
}
