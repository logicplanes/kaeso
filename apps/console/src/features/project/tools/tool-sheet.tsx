import { useQuery } from "@tanstack/react-query";
import { Badge, Banner, Button, Empty, JsonView, KeyValue, Section, Sheet, Skeleton, Status, Text } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import type { ProjectToolView } from "~/api/generated/types";
import { projectToolsQuery } from "~/api/queries/tools";
import { useOrgRights } from "~/auth/session";
import { t } from "~/i18n";
import { hintLabel, hintsOf } from "~/kaeso/mcp-servers";
import { stateOf, stateWord } from "./tools-tab";

export function ToolSheet({ org, project, toolId, onClose }: { org: string; project: string; toolId: string | undefined; onClose: () => void }) {
  const tools = useQuery(projectToolsQuery(org, project));
  const tool = tools.data?.find((x) => x.id === toolId);
  return (
    <Sheet
      open={!!toolId}
      onOpenChange={(o) => !o && onClose()}
      modal={false}
      size="md"
      title={tool?.exposed_name ?? t("project.tools.tool", "Tool")}
      {...(tool ? { subtitle: tool.server_name } : {})}
    >
      {toolId && (tools.isPending ? <Skeleton lines={4} /> : tool ? <ToolBody key={tool.id} org={org} project={project} tool={tool} /> : <Gone />)}
    </Sheet>
  );
}

function Gone() {
  return <Empty title={t("project.tools.gone", "This tool isn't in the project anymore.")} />;
}

function ToolBody({ org, project, tool }: { org: string; project: string; tool: ProjectToolView }) {
  const orgRights = useOrgRights(org);
  const state = stateWord(stateOf(tool));
  const hints = hintsOf(tool.annotations);
  return (
    <div className="flex flex-col gap-24">
      {tool.state === "changed" && (
        <Banner
          tone="warning"
          title={t("project.tools.changedTitle", "{server} changed this tool", { server: tool.server_name })}
          {...(orgRights.has("mcp_servers.view")
            ? {
                actions: (
                  <Button size="sm" href={`/orgs/${org}/mcp-servers?server=${tool.server}`}>
                    {t("project.tools.review", "Review the change")}
                  </Button>
                ),
              }
            : {})}
        >
          {t("project.tools.changedHint", "The portal offers it again once an admin approves the change.")}
        </Banner>
      )}
      <KeyValue
        items={[
          { label: t("project.tools.state", "State"), value: <Status tone={state.tone}>{state.label}</Status> },
          { label: t("project.tools.onServer", "Name on the server"), value: tool.name, mono: true },
          { label: t("project.tools.server", "Server"), value: tool.server_name },
          ...(hints.length
            ? [
                {
                  label: t("project.tools.hints", "Hints"),
                  value: (
                    <span className="flex flex-wrap gap-6">
                      {hints.map((h) => (
                        <Badge key={h} tone={h === "destructive" ? "warning" : "outline"}>
                          {hintLabel(h)}
                        </Badge>
                      ))}
                    </span>
                  ),
                },
              ]
            : []),
          ...(tool.calls_7d != null
            ? [
                { label: t("project.tools.calls7d", "Calls in 7 days"), value: String(tool.calls_7d) },
                {
                  label: t("project.tools.lastCall", "Last call"),
                  value: tool.last_call_at ? formatDate(tool.last_call_at, { time: true }) : t("common.never", "Never"),
                },
              ]
            : []),
        ]}
      />
      {tool.description && (
        <Section title={t("project.tools.description", "Description")} spacing="none">
          <Text>{tool.description}</Text>
        </Section>
      )}
      <Section title={t("project.tools.inputSchema", "Input schema")} spacing="none">
        <JsonView value={tool.input_schema} defaultDepth={3} />
      </Section>
      <Section
        title={t("project.tools.whoCanUse", "Who can use it")}
        spacing="none"
        actions={
          <Button size="sm" variant="ghost" href={`/orgs/${org}/projects/${project}/logs?kind=tools&call_tool=${encodeURIComponent(tool.exposed_name)}`}>
            {t("project.tools.seeCalls", "See its calls")}
          </Button>
        }
      >
        <Text tone="secondary">
          {tool.offered
            ? t("project.tools.whoHint", "People and keys with Use tools whose tool access includes it.")
            : t("project.tools.notOffered", "Nobody: the portal doesn't offer it now.")}
        </Text>
      </Section>
    </div>
  );
}
