import { Input, Select } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export type Principal = {
  tenantId: string;
  clientId: string;
  clientSecret: string;
  scope: string | null;
};

export const emptyPrincipal: Principal = { tenantId: "", clientId: "", clientSecret: "", scope: null };

const GUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const DOMAIN = /^(?=.{1,253}$)([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/i;

export function principalBlocked(p: Principal): string | undefined {
  const tenant = p.tenantId.trim();
  if (!GUID.test(tenant) && !DOMAIN.test(tenant)) return t("kaeso.providerKey.entraTenantBlocked", "Give the tenant ID or one of its domains");
  if (!GUID.test(p.clientId.trim())) return t("kaeso.providerKey.entraClientBlocked", "Give the application (client) ID");
  if (!p.clientSecret.trim() || /\s/.test(p.clientSecret.trim())) return t("kaeso.providerKey.entraSecretBlocked", "Paste the client secret's value");
  return undefined;
}

/** The secret the Platform API takes for a service principal: one JSON object. */
export function principalSecret(p: Principal, scopes: readonly string[]): string {
  return JSON.stringify({
    tenant_id: p.tenantId.trim(),
    client_id: p.clientId.trim(),
    client_secret: p.clientSecret.trim(),
    scope: p.scope ?? scopes[0],
  });
}

export function EntraIdFields({ value, onValue, scopes }: { value: Principal; onValue: (p: Principal) => void; scopes: readonly string[] }) {
  const field = (name: "tenantId" | "clientId" | "clientSecret") => ({
    value: value[name],
    onChange: (e: { target: { value: string } }) => onValue({ ...value, [name]: e.target.value }),
    autoComplete: "off",
    spellCheck: false,
  });
  return (
    <>
      <Input
        label={t("kaeso.providerKey.entraTenant", "Directory (tenant) ID")}
        placeholder={t("kaeso.providerKey.entraTenantExample", "aaaabbbb-0000-cccc-1111-dddd2222eeee")}
        {...field("tenantId")}
        autoFocus
      />
      <Input
        label={t("kaeso.providerKey.entraClient", "Application (client) ID")}
        placeholder={t("kaeso.providerKey.entraClientExample", "00001111-aaaa-2222-bbbb-3333cccc4444")}
        {...field("clientId")}
      />
      <Input secret label={t("kaeso.providerKey.entraSecret", "Client secret")} {...field("clientSecret")} />
      <Select
        fullWidth
        label={t("kaeso.providerKey.entraScope", "Token scope")}
        description={t("kaeso.providerKey.entraScopeHint", "Keep the first unless your resource asks for the other.")}
        value={value.scope ?? scopes[0] ?? null}
        onValueChange={(v) => onValue({ ...value, scope: v as string })}
        options={scopes.map((s) => ({ value: s, label: s }))}
      />
    </>
  );
}
