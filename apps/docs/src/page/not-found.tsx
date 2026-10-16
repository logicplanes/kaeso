import { AppFrame, Button, Empty, VisuallyHidden } from "@logicplanes/kaeso-ui";
import { homePath } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";
import { usePageTitle } from "~/shell/title";

export function NotFound() {
  const t = useText();
  const locale = useSiteLocale();
  usePageTitle(t("notFound.title"));
  return (
    <AppFrame.Body className="relative">
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
      />
    </AppFrame.Body>
  );
}
