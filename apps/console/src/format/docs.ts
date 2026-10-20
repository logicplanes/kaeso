import { locale } from "~/i18n";

export function docsPage(path: string): string {
  const current = locale();
  return `https://docs.kaeso.app${current === "en" ? "" : `/${current}`}${path}`;
}
