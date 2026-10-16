import { useEffect } from "react";
import { useText } from "~/i18n/text";
import { documentTitle } from "~/page/document-title";

export function usePageTitle(title: string): void {
  const siteName = useText()("site.name");
  useEffect(() => {
    document.title = documentTitle(title, siteName);
  }, [title, siteName]);
}
