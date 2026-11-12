import { AppFrame, Button, Empty, VisuallyHidden } from "@logicplanes/kaeso-ui";
import { homePath } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import { LegalLinks } from "~/shell/legal-links";
import { usePageTitle } from "~/shell/title";

export function NotFound() {
  const t = useText();
  const locale = useSiteLocale();
  usePageTitle(t("notFound.title"));
  return (
    <AppFrame.Body className="relative flex flex-col">
      <AppFrame.MenuButton className="absolute top-10 left-12 z-10" />
      <VisuallyHidden>
        <h1>{t("notFound.title")}</h1>
      </VisuallyHidden>
      <Empty
        size="lg"
        title={t("notFound.heading")}
        description={t("notFound.description")}
        actions={
          <Button variant="primary" href={homePath(locale)}>
            {t("notFound.action")}
          </Button>
        }
        className="flex-1"
      />
      <LegalLinks className="mx-24 mb-24 justify-center" />
    </AppFrame.Body>
  );
}
