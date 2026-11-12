import { Input, Link } from "@logicplanes/kaeso-ui";
import type { KeyPageView } from "~/api/generated/types";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";

/** A Cloudflare account ID: 32 hexadecimal characters, in either case as pasted. */
export function isAccountId(value: string): boolean {
  return /^[0-9a-f]{32}$/i.test(value.trim());
}

/** A gateway ID: 1 to 64 letters, digits, hyphens and underscores, starting with a letter or digit. */
export function isGatewayId(value: string): boolean {
  return /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/.test(value.trim());
}

/** The account a key of a provider whose API names the account calls, and the gateway its requests go through. */
export function AccountScopeFields({
  page,
  accountId,
  onAccountId,
  gatewayId,
  onGatewayId,
}: {
  page: KeyPageView;
  accountId: string;
  onAccountId: (value: string) => void;
  gatewayId: string;
  onGatewayId: (value: string) => void;
}) {
  const accountInvalid = accountId.trim() !== "" && !isAccountId(accountId);
  const gatewayInvalid = gatewayId.trim() !== "" && !isGatewayId(gatewayId);
  return (
    <>
      <Input
        label={t("kaeso.providerKey.accountId", "Account ID")}
        description={tRich("kaeso.providerKey.accountIdHint", "The account the key's requests are billed to. {link}", {
          link: (
            <Link href={page.url} external>
              {t("kaeso.providerKey.accountIdWhere", "Where to find it")}
            </Link>
          ),
        })}
        placeholder={t("kaeso.providerKey.accountIdExample", "0123456789abcdef0123456789abcdef")}
        value={accountId}
        onChange={(e) => onAccountId(e.target.value)}
        invalid={accountInvalid}
        {...(accountInvalid ? { error: t("kaeso.providerKey.accountIdInvalid", "An account ID is 32 characters, 0–9 and a–f.") } : {})}
        autoComplete="off"
        spellCheck={false}
      />
      <Input
        label={t("kaeso.providerKey.gatewayId", "Gateway (optional)")}
        description={t(
          "kaeso.providerKey.gatewayIdHint",
          "Requests go through this gateway, with its logs, caching and routes. Leave empty for the account's default gateway.",
        )}
        placeholder={t("kaeso.providerKey.gatewayIdExample", "default")}
        value={gatewayId}
        onChange={(e) => onGatewayId(e.target.value)}
        invalid={gatewayInvalid}
        {...(gatewayInvalid ? { error: t("kaeso.providerKey.gatewayIdInvalid", "Use letters, digits, hyphens and underscores, up to 64.") } : {})}
        autoComplete="off"
        spellCheck={false}
        maxLength={64}
      />
    </>
  );
}
