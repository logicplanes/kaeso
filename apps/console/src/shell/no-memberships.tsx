import { AppFrame, Button, Empty, PageHeader } from "@logicplanes/kaeso-ui";
import { useSession } from "~/auth/session";
import { t } from "~/i18n";
import { usePageTitle } from "./title";
import { openCreateOrg } from "./create-org-host";

export function NoMemberships() {
  usePageTitle(t("shell.welcome.pageTitle", "Welcome"));
  const session = useSession();
  return (
    <>
      <PageHeader start={<AppFrame.MenuButton />} title={t("shell.welcome.title", "Welcome to Kaeso")} />
      <AppFrame.Body>
        {session.can_create_orgs ? (
          <Empty
            size="lg"
            title={t("shell.welcome.noOrg", "You're not in an organization yet")}
            description={t("shell.welcome.createOrAsk", "Create one to start, or ask an owner or admin to invite you to theirs.")}
            actions={
              <Button variant="primary" onClick={openCreateOrg}>
                {t("shell.createOrg", "Create organization")}
              </Button>
            }
          />
        ) : (
          <Empty
            size="lg"
            title={t("shell.welcome.noOrg", "You're not in an organization yet")}
            description={t("shell.welcome.ask", "Ask an owner or admin of an organization to invite you.")}
          />
        )}
      </AppFrame.Body>
    </>
  );
}
