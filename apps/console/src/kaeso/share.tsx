import { Suspense, lazy } from "react";
import type { ShareTarget } from "./share-button";

// The button loads with its page's other lazy parts, so the pages that offer it keep their bundle budgets.
const ShareButton = lazy(() => import("./share-button").then((m) => ({ default: m.ShareButton })));

export function Share({ org, target }: { org: string; target: ShareTarget }) {
  return (
    <Suspense fallback={null}>
      <ShareButton org={org} target={target} />
    </Suspense>
  );
}

/** The last `days` days up to today, UTC, as a period to share. */
export function lastDays(days: number, now = new Date()): { from: string; to: string } {
  const to = now.toISOString().slice(0, 10);
  const from = new Date(Date.parse(`${to}T00:00:00Z`) - (days - 1) * 86_400_000).toISOString().slice(0, 10);
  return { from, to };
}
