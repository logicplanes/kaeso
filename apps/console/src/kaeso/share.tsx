import { Suspense, lazy } from "react";
import { Button, icons } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import { usePhone } from "~/shell/use-phone";
import type { ShareTarget } from "./share-button";

// The button loads with its page's other lazy parts, so the pages that offer it keep their bundle budgets.
const ShareButton = lazy(() => import("./share-button").then((m) => ({ default: m.ShareButton })));

export function ShareTrigger({
  compact,
  disabled,
  disabledReason,
  onClick,
}: {
  compact?: boolean | undefined;
  disabled: boolean;
  disabledReason?: string | undefined;
  onClick?: (() => void) | undefined;
}) {
  const phone = usePhone();
  const state = { disabled, ...(disabledReason ? { disabledReason } : {}), ...(onClick ? { onClick } : {}) };
  return compact && phone ? (
    <Button size="sm" icon={icons.alias} aria-label={t("kaeso.share.share", "Share")} {...state} />
  ) : (
    <Button size="sm" leadingIcon={icons.alias} {...state}>
      {t("kaeso.share.share", "Share")}
    </Button>
  );
}

export function Share({ org, target, compact }: { org: string; target: ShareTarget; compact?: boolean | undefined }) {
  return (
    // While its code loads, the button stands disabled in its place, as it does until the link policy is in, so the header doesn't move.
    <Suspense fallback={<ShareTrigger compact={compact} disabled />}>
      <ShareButton org={org} target={target} compact={compact} />
    </Suspense>
  );
}

/** The last `days` days up to today, UTC, as a period to share. */
export function lastDays(days: number, now = new Date()): { from: string; to: string } {
  const to = now.toISOString().slice(0, 10);
  const from = new Date(Date.parse(`${to}T00:00:00Z`) - (days - 1) * 86_400_000).toISOString().slice(0, 10);
  return { from, to };
}
