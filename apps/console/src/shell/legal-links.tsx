import { useQuery } from "@tanstack/react-query";
import { Link, Text } from "@logicplanes/kaeso-ui";
import type { LegalLinksView } from "~/api/generated/types";
import { signInMethodsQuery } from "~/auth/sign-in-api";
import { t } from "~/i18n";

export const anyLegalLink = (legal: LegalLinksView | undefined): legal is LegalLinksView => !!(legal?.imprint_url || legal?.privacy_url);

export function PublicLegalLinks() {
  const legal = useQuery(signInMethodsQuery).data?.legal;
  if (!anyLegalLink(legal)) return null;
  return (
    <Text as="nav" variant="small" tone="secondary" aria-label={t("shell.legal.label", "Legal")} className="flex flex-wrap justify-center gap-x-16 gap-y-8">
      {legal.imprint_url && (
        <Link href={legal.imprint_url} variant="quiet" external>
          {t("shell.legal.imprint", "Legal notice")}
        </Link>
      )}
      {legal.privacy_url && (
        <Link href={legal.privacy_url} variant="quiet" external>
          {t("shell.legal.privacy", "Privacy")}
        </Link>
      )}
    </Text>
  );
}
