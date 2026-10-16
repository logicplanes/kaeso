import { AppFrame, Button, Empty, PageHeader } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import { readLast } from "./last";
import { placePath } from "./pages";
import { usePageTitle } from "./title";

export function NotFound() {
  usePageTitle(t("shell.notFound", "Not found"));
  const last = readLast();
  return (
    <>
      <PageHeader start={<AppFrame.MenuButton />} title={t("shell.notFound", "Not found")} />
      <AppFrame.Body>
        <Empty
          size="lg"
          title={t("shell.notFound.title", "This page doesn't exist.")}
          description={t("shell.notFound.description", "The link may be old, or the page moved.")}
          actions={
            <Button variant="primary" href={last ? placePath(last) : "/"}>
              {t("shell.goToOverview", "Go to the overview")}
            </Button>
          }
        />
      </AppFrame.Body>
    </>
  );
}
