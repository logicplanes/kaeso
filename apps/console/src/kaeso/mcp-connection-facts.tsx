import { KeyValue } from "@logicplanes/kaeso-ui";
import { formatRelative } from "@logicplanes/kaeso-ui/format";
import type { McpConnectionView } from "~/api/generated/types";
import { t } from "~/i18n";

/** A connection to an MCP server, read the same way on the org's server sheet and under Account › Connections. */
export function McpConnectionFacts({ connection: c }: { connection: McpConnectionView }) {
  return (
    <KeyValue
      items={[
        ...(c.header_name ? [{ label: t("orgHome.mcp.header", "Header"), value: `${c.header_name}: …${c.last4}`, mono: true }] : []),
        { label: t("orgHome.mcp.signedIn", "Signed in"), value: formatRelative(c.updated_at) },
        ...(c.expires_at ? [{ label: t("orgHome.mcp.renews", "Renews"), value: formatRelative(c.expires_at) }] : []),
        { label: t("common.lastUsed", "Last used"), value: c.last_used_at ? formatRelative(c.last_used_at) : t("common.never", "Never") },
      ]}
    />
  );
}
