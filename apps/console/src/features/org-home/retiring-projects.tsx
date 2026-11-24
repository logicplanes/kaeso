import { useQuery } from "@tanstack/react-query";
import { Banner, Link } from "@logicplanes/kaeso-ui";
import { orgRetirementsQuery } from "~/api/queries/projects";
import { fullDayLabel } from "~/format/period";
import { t } from "~/i18n";

const SHOWN = 5;

/** The org's projects that use models their providers retire, on the overview. */
export function RetiringProjects({ org }: { org: string }) {
  const retiring = useQuery(orgRetirementsQuery(org));
  const list = retiring.data?.data ?? [];
  if (list.length === 0) return null;
  return (
    <Banner
      tone="warning"
      className="mt-48"
      title={t("orgHome.retiring.title", "{count, plural, one {# project uses retiring models} other {# projects use retiring models}}", {
        count: list.length,
      })}
    >
      <ul className="flex flex-col gap-4">
        {list.slice(0, SHOWN).map((p) => (
          <li key={p.project_id}>
            <Link href={`/orgs/${org}/projects/${p.project_id}/models?tab=retiring`}>{p.name}</Link>
            {" · "}
            {p.next_shutdown_on
              ? t("orgHome.retiring.next", "{count, plural, one {# model} other {# models}}, the first shuts down {date}", {
                  count: p.models,
                  date: fullDayLabel(p.next_shutdown_on),
                })
              : p.retired > 0
                ? t("orgHome.retiring.retired", "{count, plural, one {# model} other {# models}}, {retired} already retired", {
                    count: p.models,
                    retired: p.retired,
                  })
                : t("orgHome.retiring.noDate", "{count, plural, one {# model} other {# models}}, no shutdown date yet", { count: p.models })}
          </li>
        ))}
        {list.length > SHOWN && (
          <li>{t("orgHome.retiring.more", "{count, plural, one {# more project} other {# more projects}}", { count: list.length - SHOWN })}</li>
        )}
      </ul>
    </Banner>
  );
}
