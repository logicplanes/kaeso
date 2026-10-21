import { InsetContent, Section, SkeletonLine } from "@logicplanes/kaeso-ui";
import { useProjectRights } from "~/auth/rights";
import { t } from "~/i18n";

export function AsidePlaceholder({ org, project }: { org: string; project: string }) {
  const rights = useProjectRights(org, project);
  return (
    <InsetContent>
      {rights.has("limits.view") && (
        <Section title={t("project.overview.limits", "Limits")} spacing="none">
          <SkeletonLine lines={2} />
        </Section>
      )}
      {rights.has("logs.metadata.view") && (
        <Section title={t("project.overview.recentErrors", "Recent errors")} spacing="none">
          <SkeletonLine lines={3} />
        </Section>
      )}
      {(rights.has("models.view") || rights.has("keys.all.view")) && (
        <Section title={t("project.overview.thisProject", "This project")} spacing="none">
          <SkeletonLine lines={2} />
        </Section>
      )}
    </InsetContent>
  );
}
