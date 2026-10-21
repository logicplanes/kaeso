import { Suspense, lazy, useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "~/api/client";
import { whatsNewQuery } from "~/api/queries/whats-new";
import { useWhatsNewAsked } from "./whats-new-request";

const WhatsNew = lazy(() => import("./whats-new-dialog"));

export function WhatsNewHost() {
  const news = useQuery(whatsNewQuery);
  const firstVisit = news.data?.seen === null;
  useEffect(() => {
    if (firstVisit) void api.POST("/platform/v1/me/whats-new/seen");
  }, [firstVisit]);
  const asked = useWhatsNewAsked();
  const [askedBefore] = useState(asked);
  const unseen = news.data?.releases ?? [];
  const releases = unseen.length ? unseen : asked > askedBefore && news.data ? [news.data.running] : [];
  if (!releases.length) return null;
  return (
    <Suspense fallback={null}>
      <WhatsNew key={asked} releases={releases} />
    </Suspense>
  );
}
