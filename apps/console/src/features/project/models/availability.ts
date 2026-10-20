import { formatDate } from "@logicplanes/kaeso-ui/format";
import type { CatalogueModelView, CheckFailure } from "~/api/generated/types";
import { t } from "~/i18n";

export function checkFailureLabel(f: CheckFailure, provider: string): string {
  switch (f.reason) {
    case "no_key":
      return t("models.check.noKey", "No {provider} key is set up for this project", { provider });
    case "key_refused":
      return t("models.check.keyRefused", "{provider} refused the project's provider key", { provider });
    case "inconclusive":
      return t("models.check.inconclusive", "{provider} can't confirm whether it serves the model", { provider });
    case "key_changed":
      return t("models.check.keyChanged", "The {provider} key changed during the check; check again", { provider });
    default:
      return t("models.check.providerError", "{provider} didn't answer; try again shortly", { provider });
  }
}

export function availabilityLabel(m: Pick<CatalogueModelView, "availability" | "availability_checked_at">): string {
  const date = m.availability_checked_at ? formatDate(m.availability_checked_at) : null;
  switch (m.availability) {
    case "available":
      return date ? t("models.availableSince", "Available, checked {date}", { date }) : t("models.available", "Available");
    case "unavailable":
      return date ? t("models.unavailableSince", "Unavailable since {date}", { date }) : t("models.unavailable", "Unavailable");
    default:
      return t("models.unchecked", "Not checked");
  }
}

export function unavailableHint(provider: string): string {
  return t("models.unavailableHint", "{provider} doesn't serve it to this project's provider key; check the key's access or remove the model.", {
    provider,
  });
}
