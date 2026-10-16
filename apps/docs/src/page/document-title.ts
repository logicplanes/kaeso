export function documentTitle(title: string, siteName: string): string {
  return title === siteName ? title : `${title} · ${siteName}`;
}
