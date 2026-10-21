import { formatDate } from "@logicplanes/kaeso-ui/format";
import type { KeyView } from "~/api/generated/types";
import { t } from "~/i18n";

const DAY = 86_400_000;

const daysLeft = (expiresAt: string) => Math.ceil((Date.parse(expiresAt) - Date.now()) / DAY);

export function expiryText(k: Pick<KeyView, "expires_at" | "state">): { text: string; soon: boolean } {
  if (!k.expires_at) return { text: t("keys.neverExpires", "Never expires"), soon: false };
  if (Date.parse(k.expires_at) <= Date.now()) return { text: t("keys.expiredOn", "Expired {date}", { date: formatDate(k.expires_at) }), soon: false };
  const days = daysLeft(k.expires_at);
  return { text: t("keys.expiresIn", "Expires in {days, plural, one {# day} other {# days}}", { days }), soon: days < 7 };
}

export function expiryDetail(expiresAt: string): string {
  const date = formatDate(expiresAt);
  if (Date.parse(expiresAt) <= Date.now()) return t("keys.expiredDetail", "{date} (expired)", { date });
  return t("keys.expiresDetail", "{date} (expires in {days, plural, one {# day} other {# days}})", { date, days: daysLeft(expiresAt) });
}
