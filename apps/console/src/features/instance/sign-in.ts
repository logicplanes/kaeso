import type { InstanceOrgView } from "~/api/generated/types";
import { t } from "~/i18n";

/** What an organization requires at sign-in, as the instance's lists and pages say it. */
export function orgSignIn(o: Pick<InstanceOrgView, "require_sso" | "require_2fa">): string {
  if (o.require_sso && o.require_2fa) return t("instance.ssoAnd2fa", "Single sign-on and two-factor");
  if (o.require_sso) return t("instance.sso", "Single sign-on");
  if (o.require_2fa) return t("instance.twoFactorAuth", "Two-factor authentication");
  return t("common.none", "None");
}
