import { useProjectRights } from "~/auth/rights";
import { lastDays } from "~/kaeso/share";
import { ShareButton } from "~/kaeso/share-button";

export function ShareCard({ org, project }: { org: string; project: string }) {
  const rights = useProjectRights(org, project);
  return <ShareButton org={org} target={{ kind: "project_card", project, ...lastDays(30), costs: rights.has("costs.view") }} />;
}
