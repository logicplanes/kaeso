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
    <Text as="nav" variant="small" tone="secondary" aria-label={docsText(lang, "Legal")} className="flex flex-wrap justify-center gap-x-16 gap-y-8">
      <Link href={kaesoApp(lang, "/impressum")} variant="quiet">
        {docsText(lang, "Legal notice")}
      </Link>
      <Link href={kaesoApp(lang, "/datenschutz")} variant="quiet">
        {docsText(lang, "Privacy")}
      </Link>
    </Text>
  );
}
