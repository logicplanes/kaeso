import { useState } from "react";
import { Button, ClientButton, CodeBlock, Dialog, Text, type ClientMark } from "@logicplanes/kaeso-ui";
import { docsPage } from "~/format/docs";
import { t } from "~/i18n";

export const mcpUrl = (): string => `${window.location.origin}/mcp`;

const VSCODE_CONFIG = ".vscode/mcp.json";
const CLIENTS: readonly ClientMark[] = ["claude", "cursor", "github-copilot"];

const clientName = (client: ClientMark): string => (client === "claude" ? "Claude Code" : client === "cursor" ? "Cursor" : "GitHub Copilot");

// Install links as Cursor and VS Code document them: https://cursor.com/docs/context/mcp/install-links and
// https://code.visualstudio.com/docs/copilot/guides/mcp-developer-guide
const cursorLink = (): string => `cursor://anysphere.cursor-deeplink/mcp/install?name=kaeso&config=${btoa(JSON.stringify({ url: mcpUrl() }))}`;
const vscodeLink = (): string => `vscode:mcp/install?${encodeURIComponent(JSON.stringify({ name: "kaeso", type: "http", url: mcpUrl() }))}`;

function Steps({ client }: { client: ClientMark }) {
  const origin = window.location.origin;
  if (client === "claude")
    return (
      <div className="flex flex-col gap-12">
        <Text variant="small" tone="secondary">
          {t("account.connectClaudeServer", "Connect the server, then run /mcp to sign in.")}
        </Text>
        <CodeBlock copy lang="bash" code={`claude mcp add --transport http --scope user kaeso ${mcpUrl()}`} />
        <Text variant="small" tone="secondary">
          {t("account.connectClaudePlugin", "The plugin adds Kaeso's skill, which teaches your agent the tools and can connect the server too.")}
        </Text>
        <CodeBlock copy lang="text" code="/plugin install kaeso --marketplace logicplanes/kaeso" />
      </div>
    );
  if (client === "cursor")
    return (
      <div className="flex flex-col gap-12">
        <Button variant="primary" fullWidth href={cursorLink()} target="_self">
          {t("account.addToCursor", "Add to Cursor")}
        </Button>
        <Text variant="small" tone="secondary">
          {t("account.connectCursorHint", "Or install Kaeso from the Cursor Marketplace and enter {origin} as KAESO_URL.", { origin })}
        </Text>
      </div>
    );
  return (
    <div className="flex flex-col gap-12">
      <Button variant="primary" fullWidth href={vscodeLink()} target="_self">
        {t("account.addToVsCode", "Add to VS Code")}
      </Button>
      <CodeBlock copy title={VSCODE_CONFIG} lang="json" code={JSON.stringify({ servers: { kaeso: { type: "http", url: mcpUrl() } } }, null, 2)} />
    </div>
  );
}

export function ConnectClients() {
  const [client, setClient] = useState<ClientMark | null>(null);
  const [shown, setShown] = useState<ClientMark>("claude");
  return (
    <>
      <span className="flex gap-8">
        {CLIENTS.map((c) => (
          <ClientButton
            key={c}
            client={c}
            aria-label={t("account.connectClient", "Add to {client}", { client: clientName(c) })}
            onClick={() => {
              setShown(c);
              setClient(c);
            }}
          />
        ))}
      </span>
      <Dialog
        open={client !== null}
        onOpenChange={(open) => !open && setClient(null)}
        title={t("account.connectTitle", "Add Kaeso to {client}", { client: clientName(shown) })}
        description={t("account.connectLead", "Your agent signs in through your browser, and you choose what it may do.")}
        footer={
          <>
            <Button href={docsPage("/api/mcp")}>{t("account.connectDocs", "Read the documentation")}</Button>
            <Dialog.Close>
              <Button variant="primary">{t("common.finish", "Finish")}</Button>
            </Dialog.Close>
          </>
        }
      >
        <Steps client={shown} />
      </Dialog>
    </>
  );
}
