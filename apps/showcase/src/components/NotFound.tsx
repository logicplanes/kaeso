import { Button, Empty } from "@logicplanes/kaeso-ui";
import { DocsLocale, docsText, useDocsLang } from "~/lib/locale";

export default function NotFound() {
  return (
    <DocsLocale>
      <Message />
    </DocsLocale>
  );
}

function Message() {
  const { lang } = useDocsLang();
  const t = (s: string) => docsText(lang, s);
  return (
    <Empty
      size="lg"
      title={t("This page doesn't exist.")}
      description={t("The link may be old, or the page moved.")}
      actions={
        <Button variant="primary" href="/">
          {t("Go to the overview")}
        </Button>
      }
    />
  );
}
