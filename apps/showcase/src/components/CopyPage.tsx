import { Button, icons, useCopy } from "@logicplanes/kaeso-ui";
import { DocsLocale, docsText, useDocsLang } from "~/lib/locale";

export default function CopyPage({ markdown }: { markdown: string }) {
  return (
    <DocsLocale>
      <CopyButton markdown={markdown} />
    </DocsLocale>
  );
}

function CopyButton({ markdown }: { markdown: string }) {
  const { copied, copy } = useCopy();
  const { lang } = useDocsLang();
  const label = docsText(lang, "Copy page");
  const done = docsText(lang, "Copied");
  return (
    <Button size="sm" leadingIcon={copied ? icons.check : icons.copy} reserveLabels={[label, done]} onClick={() => void copy(markdown)}>
      {copied ? done : label}
    </Button>
  );
}
