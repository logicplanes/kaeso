import { useState } from "react";
import { Button, icons } from "@logicplanes/kaeso-ui";
import { DocsLocale, docsText, useDocsLang } from "~/lib/locale";

export default function CopyPage({ markdown }: { markdown: string }) {
  return (
    <DocsLocale>
      <CopyButton markdown={markdown} />
    </DocsLocale>
  );
}

function CopyButton({ markdown }: { markdown: string }) {
  const [copied, setCopied] = useState(false);
  const { lang } = useDocsLang();
  const label = docsText(lang, "Copy page");
  const done = docsText(lang, "Copied");
  return (
    <Button
      size="sm"
      leadingIcon={copied ? icons.check : icons.copy}
      reserveLabels={[label, done]}
      onClick={async () => {
        await navigator.clipboard.writeText(markdown);
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      }}
    >
      {copied ? done : label}
    </Button>
  );
}
