import { useEffect } from "react";

export function usePageTitle(page: string, context?: string): void {
  useEffect(() => {
    document.title = [page, context, "Kaeso"].filter(Boolean).join(" · ");
  }, [page, context]);
}
