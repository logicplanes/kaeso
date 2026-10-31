import { AppFrame, Empty } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import { ProjectHeader } from "~/shell/project-header";

export function LogsOff({ org, project }: { org: string; project: string }) {
  return (
    <>
      <ProjectHeader org={org} project={project} page={t("logs.title", "Request logs")} path="logs" />
      <AppFrame.Body padding="page">
        <Empty
          size="lg"
          title={t("logs.off", "Request logs are off on this install")}
          description={t("logs.offHint", "Usage, limits and budgets still count every request.")}
        />
      </AppFrame.Body>
    </>
  );
}
