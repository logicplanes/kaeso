import { Suspense, lazy, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "~/api/client";
import { whatsNewQuery } from "~/api/queries/whats-new";

const WhatsNew = lazy(() => import("./whats-new-dialog"));

export function WhatsNewHost() {
  const news = useQuery(whatsNewQuery);
  const firstVisit = news.data?.seen === null;
  useEffect(() => {
    if (firstVisit) void api.POST("/platform/v1/me/whats-new/seen");
  }, [firstVisit]);
  const releases = news.data?.releases ?? [];
  if (!releases.length) return null;
  return (
    <Suspense fallback={null}>
      <WhatsNew releases={releases} />
    </Suspense>
  );
}
