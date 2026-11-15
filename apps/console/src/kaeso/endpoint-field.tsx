import { Input, Link } from "@logicplanes/kaeso-ui";
import type { KeyEndpointView } from "~/api/generated/types";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";

const PASTED_PATH = /^\/openai(\/v1)?\/?$/i;

/** Why an endpoint isn't one of the provider's, as the gateway checks it, or null when it is. */
export function endpointProblem(value: string, rule: KeyEndpointView): string | null {
  let url: URL;
  try {
    url = new URL(value.trim());
  } catch {
    return t("kaeso.providerKey.endpointNotUrl", "Paste the endpoint as a URL, starting with https://.");
  }
  if (url.protocol !== "https:" || url.username || url.password || url.search || url.hash)
    return t("kaeso.providerKey.endpointHttps", "Use an https address without a user name, query or fragment.");
  const host = url.hostname.toLowerCase();
  const resource = rule.hosts.map((suffix) => (host.endsWith(suffix) ? host.slice(0, -suffix.length) : null)).find((r) => r !== null);
  if (!resource || url.port || !/^[a-z0-9][a-z0-9.-]*$/.test(host))
    return t("kaeso.providerKey.endpointHost", "Use the resource's address, such as {example}.", { example: rule.example });
  if (url.pathname !== "/" && !PASTED_PATH.test(url.pathname)) return t("kaeso.providerKey.endpointPath", "Leave out the path; Kaeso adds the API's own.");
  return null;
}

/** The endpoint of the resource a key of a provider whose keys each belong to one resource calls. */
export function EndpointField({ rule, value, onValue }: { rule: KeyEndpointView; value: string; onValue: (value: string) => void }) {
  const problem = value.trim() === "" ? null : endpointProblem(value, rule);
  return (
    <Input
      label={t("kaeso.providerKey.endpoint", "Endpoint")}
      description={tRich("kaeso.providerKey.endpointHint", "The address of the resource the key belongs to, shown beside the key. {link}", {
        link: (
          <Link href={rule.page.url} external>
            {t("kaeso.providerKey.endpointWhere", "Where to find it")}
          </Link>
        ),
      })}
      placeholder={rule.example}
      value={value}
      onChange={(e) => onValue(e.target.value)}
      invalid={problem !== null}
      {...(problem ? { error: problem } : {})}
      type="url"
      inputMode="url"
      autoComplete="off"
      spellCheck={false}
      maxLength={2048}
    />
  );
}
