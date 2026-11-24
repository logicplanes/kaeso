import { useQuery } from "@tanstack/react-query";
import { ShareBar } from "@logicplanes/kaeso-ui";
import { orgProjectsQuery } from "~/api/queries/projects";
import { monthByProjectQuery } from "~/api/queries/usage";
import { utcDay } from "~/format/period";
import { t } from "~/i18n";
import { useOrgMoney } from "~/kaeso/org-money";

const REFRESH = 30_000;

export function CostByProject({ org }: { org: string }) {
  const byProject = useQuery({ ...monthByProjectQuery(org, utcDay(new Date())), refetchInterval: REFRESH });
  const projects = useQuery(orgProjectsQuery(org, true));
  const money = useOrgMoney(org);
  const slices = [...(byProject.data ?? [])]
    .filter((r) => Number(r.converted_cost ?? 0) > 0)
    .sort((a, b) => Number(b.converted_cost) - Number(a.converted_cost))
    .slice(0, 4)
    .map((r) => ({ id: r.key, label: projects.data?.find((p) => p.id === r.key)?.name ?? r.key, value: Number(r.converted_cost) }));
  if (!slices.length) return null;
  return <ShareBar aria-label={t("kaeso.overview.costByProject", "Cost by project")} slices={slices} formatValue={(v) => money.period(v.toFixed(6))} />;
}
