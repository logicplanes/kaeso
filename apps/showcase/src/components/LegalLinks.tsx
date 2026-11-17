import { Link, Text } from "@logicplanes/kaeso-ui";
import { DocsLocale, docsText, useDocsLang } from "~/lib/locale";
import { kaesoApp } from "~/lib/nav";

export default function LegalLinks() {
  return (
    <DocsLocale>
      <Links />
    </DocsLocale>
  );
}

function Links() {
  const { lang } = useDocsLang();
  return (
    <div className="flex flex-col gap-8 pt-20 divider-t sm:flex-row sm:items-center sm:justify-between sm:gap-24">
      <Text as="p" variant="small" tone="tertiary">
        Kaeso UI
      </Text>
      <Text as="nav" variant="small" aria-label={docsText(lang, "Legal")} className="flex flex-wrap items-center gap-x-20 gap-y-8">
        <Link href={kaesoApp(lang, "/impressum")} variant="quiet">
          {docsText(lang, "Legal notice")}
        </Link>
        <Link href={kaesoApp(lang, "/datenschutz")} variant="quiet">
          {docsText(lang, "Privacy")}
        </Link>
      </Text>
    </div>
  );
}
