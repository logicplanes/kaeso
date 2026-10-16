import { useLayoutEffect } from "react";
import { LocaleProvider } from "@logicplanes/kaeso-ui/i18n";
import { TextProvider } from "~/i18n/text";
import type { LocaleData } from "~/site/data";
import { DocsShell } from "./docs-shell";

export function LocaleShell({ data }: { data: LocaleData }) {
  useLayoutEffect(() => {
    document.documentElement.lang = data.locale;
  }, [data.locale]);
  return (
    <TextProvider locale={data.locale}>
      <LocaleProvider locale={data.locale} messages={data.ui}>
        <DocsShell locale={data.locale} nav={data.site.nav} />
      </LocaleProvider>
    </TextProvider>
  );
}
