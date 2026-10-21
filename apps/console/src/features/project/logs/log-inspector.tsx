import { useEffect, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Badge,
  Button,
  CodeBlock,
  Collapsible,
  Empty,
  InlineCopyText,
  JsonView,
  KeyValue,
  Pill,
  ProviderBadge,
  RequestTimeline,
  Section,
  Sheet,
  Skeleton,
  SkeletonLine,
  StatGrid,
  StatTile,
  StatusCode,
  Switch,
  Tabs,
  Text,
  Tooltip,
  UsageBreakdown,
} from "@logicplanes/kaeso-ui";
import { formatBytes, formatDate, formatNumber } from "@logicplanes/kaeso-ui/format";
import { isApiError } from "~/api/errors";
import type { LogEntryView } from "~/api/generated/types";
import { logEntryQuery } from "~/api/queries/logs";
import { dialectLabel, errorKindLabel } from "~/format/labels";
import { useProviders } from "~/kaeso/providers";
import { costUSD } from "~/format/money";
import { describeUserAgent } from "~/format/user-agent";
import { orgProjectsQuery } from "~/api/queries/projects";
import { catalogueQuery } from "~/api/queries/models";
import { usePartyName } from "../keys/party";
import { inputTokens, totalTokens } from "~/kaeso/tokens";
import { embeddingsView, isKnownReplyForm, replyView, requestView } from "./render";
import { seconds, startedLabel } from "./timing";
import { logMessage, useGatewayErrors } from "./messages";
import { CodeText } from "~/kaeso/code-text";
import { useRememberRecent } from "~/shell/recent";
import { useProjectRights } from "~/auth/rights";
import { useNeedsRight } from "~/auth/needs-right";
import { t } from "~/i18n";
import { common } from "~/i18n/common";

type ConversationModule = typeof import("./conversation");
let conversationModule: ConversationModule | undefined;
const loadConversation = () => import("./conversation").then((m) => (conversationModule = m));

function useConversation() {
  const [mod, setMod] = useState(conversationModule);
  useEffect(() => {
    if (mod) return;
    let live = true;
    void loadConversation().then((m) => live && setMod(m));
    return () => {
      live = false;
    };
  }, [mod]);
  return mod?.default;
}

function ConversationPending({ messages }: { messages: number }) {
  return (
    <div className="flex flex-col gap-24" aria-busy="true">
      {Array.from({ length: Math.min(messages, 4) }, (_, i) => (
        <div key={i} className="flex flex-col gap-8">
          <Skeleton shape="line" className="w-64" />
          <SkeletonLine lines={3} />
        </div>
      ))}
    </div>
  );
}

export function LogInspector({
  org,
  project,
  logId,
  keyName,
  onClose,
}: {
  org: string;
  project: string;
  logId: string | undefined;
  keyName: (id: string) => string;
  onClose: () => void;
}) {
  const entry = useQuery({ ...logEntryQuery(org, project, logId ?? ""), enabled: !!logId });
  const modelName = useModelName(org, project);
  const e = entry.data;
  return (
    <Sheet
      open={!!logId}
      onOpenChange={(o) => !o && onClose()}
      modal={false}
      size="lg"
      title={
        e ? (
          <span className="flex min-w-0 items-center gap-8">
            <StatusCode code={e.status} />
            <span className="truncate">{e.model ? modelName(e.provider, e.model) : e.endpoint}</span>
          </span>
        ) : (
          t("logs.inspector.title", "Request")
        )
      }
      subtitle={e?.model ? `${e.endpoint} · ${logId}` : logId}
      actions={e?.stream ? <Badge>{t("logs.inspector.streamed", "Streamed")}</Badge> : undefined}
    >
      {logId && <Body key={logId} org={org} project={project} logId={logId} keyName={keyName} onClose={onClose} />}
    </Sheet>
  );
}

function Body({
  org,
  project,
  logId,
  keyName,
  onClose,
}: {
  org: string;
  project: string;
  logId: string;
  keyName: (id: string) => string;
  onClose: () => void;
}) {
  const rights = useProjectRights(org, project);
  const entry = useQuery(logEntryQuery(org, project, logId));
  const projectName = useQuery(orgProjectsQuery(org)).data?.find((p) => p.id === project)?.name;
  useRememberRecent(
    entry.data
      ? {
          kind: "request",
          id: logId,
          label: entry.data.model ? `${entry.data.status} ${entry.data.endpoint} · ${entry.data.model}` : `${entry.data.status} ${entry.data.endpoint}`,
          href: `/orgs/${org}/projects/${project}/logs?log=${logId}`,
          detail: projectName
            ? t("logs.recentDetail", "{project}, {time}", { project: projectName, time: formatDate(entry.data.started_at, { time: true, seconds: true }) })
            : formatDate(entry.data.started_at, { time: true, seconds: true }),
        }
      : null,
  );
  const [tab, setTab] = useState("overview");
  const conversation = useConversation();
  if (entry.isPending) return <Skeleton className="h-full w-full" />;
  if (entry.isError) {
    return (
      <Empty
        title={
          isApiError(entry.error) && entry.error.type === "not_found"
            ? t("logs.inspector.notFound", "This request doesn't exist or was removed.")
            : t("logs.inspector.loadFailed", "Couldn't load this request.")
        }
        actions={<Button onClick={onClose}>{t("logs.inspector.back", "Back to requests")}</Button>}
      />
    );
  }
  const e = entry.data;
  const canContent = rights.has("logs.content.view");
  return (
    <Tabs value={tab} onValueChange={setTab}>
      <Tabs.List aria-label={t("logs.inspector.title", "Request")}>
        <Tabs.Tab value="overview">{t("logs.inspector.overview", "Overview")}</Tabs.Tab>
        <Tabs.Tab value="request">{t("logs.inspector.request", "Request")}</Tabs.Tab>
        <Tabs.Tab value="reply">{t("logs.inspector.reply", "Reply")}</Tabs.Tab>
        <Tabs.Tab value="raw">{t("logs.inspector.raw", "Raw")}</Tabs.Tab>
      </Tabs.List>
      <Tabs.Panel value="overview">
        <Overview org={org} project={project} e={e} keyName={keyName} />
      </Tabs.Panel>
      <Tabs.Panel value="request">{canContent ? <RequestTab e={e} Conversation={conversation} /> : <NoRight org={org} project={project} />}</Tabs.Panel>
      <Tabs.Panel value="reply">{canContent ? <ReplyTab e={e} Conversation={conversation} /> : <NoRight org={org} project={project} />}</Tabs.Panel>
      <Tabs.Panel value="raw">
        <RawTab e={e} canContent={canContent} />
      </Tabs.Panel>
    </Tabs>
  );
}

function NoRight({ org, project }: { org: string; project: string }) {
  const needs = useNeedsRight(org, project);
  return <Empty title={t("logs.inspector.noContentRight", "You can't see request content")} description={needs("logs.content.view")} />;
}

function NotLogged() {
  return (
    <Empty
      title={t("logs.inspector.notLogged", "Content not logged")}
      description={t("logs.inspector.notLoggedHint", "Logging was off when this request ran, or its content has passed the retention period.")}
    />
  );
}

function useModelName(org: string, project: string) {
  const catalogue = useQuery(catalogueQuery(org, project));
  return (provider: string, model: string) => catalogue.data?.find((m) => m.provider === provider && m.model_id === model)?.effective.display_name || model;
}

const known = (n: number | null | undefined): n is number => typeof n === "number" && Number.isFinite(n);

const STATUS_OF_KIND: Record<string, readonly number[]> = {
  invalid_request: [400],
  authentication: [401],
  permission_denied: [403],
  not_found: [404],
  request_too_large: [413],
  rate_limited: [429],
  client_disconnected: [499],
  timeout: [408, 504],
  overloaded: [529],
};

const restatesStatus = (kind: string, status: number) => !!STATUS_OF_KIND[kind]?.includes(status);

function Long({ children, mono }: { children: string; mono?: boolean }) {
  return (
    <Text truncate mono={mono} variant={mono ? "small" : "body"}>
      {children}
    </Text>
  );
}

function Named({ name, id }: { name: string; id: string }) {
  if (name === id) return <Long mono>{id}</Long>;
  return (
    <span className="flex min-w-0 items-baseline gap-8">
      <span className="shrink-0">{name}</span>
      <Text mono variant="small" tone="secondary" truncate>
        {id}
      </Text>
    </span>
  );
}

function Overview({ org, project, e, keyName }: { org: string; project: string; e: LogEntryView; keyName: (id: string) => string }) {
  const partyName = usePartyName(org, project);
  const modelName = useModelName(org, project);
  const providers = useProviders(org);
  const u = e.usage;
  const tokensIn = inputTokens(u);
  const used = totalTokens(u) > 0;
  const principal = partyName({ kind: e.principal_kind === "service_account" ? "service_account" : "user", id: e.principal_id });
  const name = modelName(e.provider, e.model);
  const sp = e.stream ? e.stream_progress : undefined;
  const message = logMessage(useGatewayErrors(), e.error_message, e.provider, providers.name);
  const embeddings = e.ingress_dialect === "openai_embeddings" || e.egress_dialect === "openai_embeddings";
  const answer = Math.max(0, u.output_tokens - u.reasoning_tokens);
  return (
    <div className="mt-24 flex flex-col gap-32">
      {!e.has_content && (
        <Text variant="small" tone="secondary">
          {t(
            "logs.inspector.notLoggedNote",
            "Content not logged: logging was off for the organization or the project, or the content has passed its retention.",
          )}
        </Text>
      )}
      {e.error_kind && (
        <Section title={t("logs.inspector.outcome", "Outcome")} spacing="none">
          <div className="-my-8 flex flex-col">
            <KeyValue
              layout="stacked"
              className="grid grid-cols-2 gap-x-32"
              items={[
                { label: common.status(), value: <StatusCode code={e.status} showText /> },
                ...(restatesStatus(e.error_kind, e.status) ? [] : [{ label: t("logs.inspector.error", "Error"), value: errorKindLabel(e.error_kind) }]),
              ]}
            />
            {message && (
              <KeyValue
                layout="stacked"
                items={[
                  {
                    label: message.provider
                      ? t("logs.inspector.messageFrom", "Message from {provider}", { provider: message.provider })
                      : t("logs.inspector.message", "Message"),
                    value: <CodeText text={message.text} />,
                    wrap: true,
                  },
                ]}
              />
            )}
          </div>
        </Section>
      )}
      <StatGrid aria-label={t("logs.inspector.summary", "Summary")}>
        <StatTile
          label={t("logs.column.duration", "Duration")}
          value={seconds(e.duration_ms)}
          detail={
            <Tooltip content={formatDate(e.started_at, { time: true, seconds: true })}>
              <time dateTime={e.started_at}>{startedLabel(e.started_at)}</time>
            </Tooltip>
          }
        />
        <StatTile
          label={t("logs.column.firstByte", "First byte")}
          value={known(e.ttfb_ms) ? seconds(e.ttfb_ms) : "—"}
          detail={known(e.ttfb_ms) ? t("logs.inspector.fromProvider", "From the provider") : t("logs.inspector.noneArrived", "None arrived")}
        />
        <StatTile
          label={common.tokens()}
          value={formatNumber(totalTokens(u))}
          detail={
            !used
              ? t("logs.inspector.noneCounted", "None counted")
              : u.reasoning_tokens
                ? t("logs.inspector.tokensInOutReasoning", "{input} in · {output} out · {reasoning} reasoning", {
                    input: formatNumber(tokensIn),
                    output: formatNumber(answer),
                    reasoning: formatNumber(u.reasoning_tokens),
                  })
                : t("logs.inspector.tokensInOut", "{input} in · {output} out", { input: formatNumber(tokensIn), output: formatNumber(u.output_tokens) })
          }
        />
        <StatTile label={common.cost()} value={costUSD(e.cost)} detail={u.estimated ? t("logs.inspector.estimated", "Estimated") : undefined} />
      </StatGrid>
      {known(e.ttfb_ms) && (
        <Section title={t("logs.inspector.timing", "Timing")} spacing="none">
          <RequestTimeline ttfbMs={e.ttfb_ms} totalMs={e.duration_ms} className="w-full" />
        </Section>
      )}
      <Section title={t("logs.inspector.route", "Route")} spacing="none">
        <KeyValue
          layout="stacked"
          className="-my-8 grid grid-cols-1 gap-x-32 sm:grid-cols-2"
          items={[
            {
              label: t("logs.inspector.format", "Format"),
              value:
                !e.egress_dialect || e.ingress_dialect === e.egress_dialect ? (
                  dialectLabel(e.ingress_dialect)
                ) : (
                  <Long>{`${dialectLabel(e.ingress_dialect)} → ${dialectLabel(e.egress_dialect)}`}</Long>
                ),
            },
            {
              label:
                e.requested_model === e.model
                  ? common.model()
                  : t("logs.inspector.modelRequestedAs", "Model, requested as {model}", { model: e.requested_model }),
              value: e.model ? <Named name={name} id={e.model} /> : "—",
            },
            {
              label: common.provider(),
              value: e.provider ? <ProviderBadge provider={e.provider} name={providers.name(e.provider)} withName size="sm" /> : "—",
            },
            {
              label: t("logs.inspector.upstreamRequest", "Upstream request"),
              value: e.upstream_request_id ? <Long mono>{e.upstream_request_id}</Long> : "—",
              ...(e.upstream_request_id ? { copy: e.upstream_request_id } : {}),
            },
          ]}
        />
      </Section>
      <Section title={t("logs.inspector.sentBy", "Sent by")} spacing="none">
        <KeyValue
          layout="stacked"
          className="-my-8 grid grid-cols-2 gap-x-32"
          items={[
            {
              label: e.principal_kind === "service_account" ? t("logs.inspector.serviceAccount", "Service account") : t("logs.filter.member", "Member"),
              value: <Long>{principal}</Long>,
            },
            e.key_id
              ? { label: t("logs.column.key", "Key"), value: <Long>{keyName(e.key_id)}</Long> }
              : { label: t("logs.filter.source", "Source"), value: t("logs.source.playground", "Playground") },
            { label: t("logs.inspector.clientIp", "Client IP"), value: <Long mono>{e.client_ip}</Long> },
            {
              label: t("logs.column.userAgent", "User agent"),
              value: e.user_agent ? <Named name={describeUserAgent(e.user_agent)} id={e.user_agent} /> : "—",
              ...(e.user_agent ? { copy: e.user_agent } : {}),
            },
          ]}
        />
      </Section>
      {sp && (
        <Section title={t("logs.inspector.stream", "Stream")} spacing="none">
          <KeyValue
            layout="stacked"
            className="-my-8 grid grid-cols-2 gap-x-32 sm:grid-cols-4"
            items={[
              { label: t("logs.inspector.events", "Events"), value: formatNumber(sp.events) },
              { label: t("logs.inspector.bytesSent", "Bytes sent"), value: formatBytes(sp.bytes) },
              { label: t("logs.inspector.firstByteSent", "First byte sent"), value: known(sp.first_byte_ms) ? seconds(sp.first_byte_ms) : common.none() },
              { label: t("logs.inspector.keepalives", "Keepalives"), value: formatNumber(sp.keepalives) },
            ]}
          />
        </Section>
      )}
      {used && (
        <Section
          title={t("logs.inspector.usage", "Usage")}
          spacing="none"
          actions={u.estimated ? <Badge tone="warning">{t("logs.inspector.estimated", "Estimated")}</Badge> : undefined}
        >
          <div className="flex flex-col gap-16">
            {u.estimated && (
              <Text variant="small" tone="secondary">
                {t("logs.inspector.estimatedHint", "The stream ended before the provider's final count; cost and limits use these numbers.")}
              </Text>
            )}
            <UsageBreakdown
              tokens={
                embeddings
                  ? { input: u.input_tokens }
                  : {
                      input: u.input_tokens,
                      cacheRead: u.cache_read_tokens,
                      cacheWrite: u.cache_write_tokens,
                      output: answer,
                      reasoning: u.reasoning_tokens,
                    }
              }
            />
          </div>
        </Section>
      )}
    </div>
  );
}

function TabHeader({ children, control }: { children: ReactNode; control?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-16 gap-y-12">
      <div className="flex min-w-0 flex-1 basis-280 items-center gap-8">{children}</div>
      {control && <div className="flex shrink-0 items-center">{control}</div>}
    </div>
  );
}

type ConversationView = ConversationModule["default"] | undefined;

function RequestTab({ e, Conversation }: { e: LogEntryView; Conversation: ConversationView }) {
  const [raw, setRaw] = useState(false);
  if (!e.content) return <NotLogged />;
  const body = e.content.request as unknown;
  const embeddings = embeddingsView(e.ingress_dialect, body);
  const view = requestView(e.ingress_dialect, body);
  const mapped = !!(view || embeddings);
  return (
    <div className="mt-24 flex flex-col gap-24">
      <TabHeader control={mapped && <Switch size="sm" label={t("logs.inspector.showRaw", "Show raw")} checked={raw} onCheckedChange={setRaw} />}>
        <Text variant="small" tone="secondary">
          {t("logs.inspector.requestAsSent", "The app's body as sent, in {format}.", { format: dialectLabel(e.ingress_dialect) })}
        </Text>
      </TabHeader>
      {raw || !mapped ? (
        <JsonView value={body} defaultDepth={4} />
      ) : embeddings ? (
        <Text>{t("logs.inspector.inputsEmbedded", "{count, plural, one {# input embedded.} other {# inputs embedded.}}", { count: embeddings.inputs })}</Text>
      ) : (
        view &&
        (Conversation ? (
          <>
            <Conversation messages={view.messages} />
            {view.tools.length > 0 && (
              <Collapsible
                title={t("logs.inspector.tools", "Tools")}
                hint={t("logs.inspector.toolCount", "{count, plural, one {# tool} other {# tools}}", { count: view.tools.length })}
              >
                <div className="flex flex-col gap-16">
                  {view.tools.map((tool, i) => (
                    <div key={`${tool.name}-${i}`} className="flex flex-col gap-8">
                      <Text mono weight="medium">
                        {tool.name}
                      </Text>
                      {tool.description && (
                        <Text variant="small" tone="secondary">
                          {tool.description}
                        </Text>
                      )}
                      {tool.schema !== undefined && <JsonView value={tool.schema} defaultDepth={2} />}
                    </div>
                  ))}
                </div>
              </Collapsible>
            )}
            {view.params.length > 0 && (
              <Section title={t("logs.inspector.parameters", "Parameters")} spacing="none">
                <KeyValue
                  layout="stacked"
                  className="-my-8 grid grid-cols-2 gap-x-32 sm:grid-cols-3"
                  items={view.params.map((p) => ({ label: p.label, value: <Long mono>{p.value}</Long> }))}
                />
              </Section>
            )}
          </>
        ) : (
          <ConversationPending messages={view.messages.length} />
        ))
      )}
    </div>
  );
}

function ReplyTab({ e, Conversation }: { e: LogEntryView; Conversation: ConversationView }) {
  const [raw, setRaw] = useState(false);
  if (!e.content) return <NotLogged />;
  const reply = e.content.reply as unknown;
  if (e.ingress_dialect === "openai_embeddings" || e.egress_dialect === "openai_embeddings") {
    return (
      <div className="mt-24">
        <Empty
          title={t("logs.inspector.noVectors", "Vectors aren't kept")}
          description={t("logs.inspector.noVectorsHint", "Embedding replies are numbers only; the log keeps their usage, not the vectors.")}
        />
      </div>
    );
  }
  if (reply === null || reply === undefined) {
    return (
      <div className="mt-24">
        <Empty
          title={t("logs.inspector.noReply", "No reply")}
          description={
            e.error_kind
              ? t("logs.inspector.failedBeforeReply", "The request failed before a reply arrived.")
              : t("logs.inspector.noReplyStored", "No reply was stored for this request.")
          }
        />
      </div>
    );
  }
  if (!isKnownReplyForm(reply)) {
    return (
      <div className="mt-24 flex flex-col gap-24">
        <TabHeader>
          <Text variant="small" tone="secondary">
            {t("logs.inspector.replyUnknownFormat", "This reply was logged in a format this console can't read yet; it's shown as stored.")}
          </Text>
        </TabHeader>
        <JsonView value={reply} defaultDepth={4} />
      </div>
    );
  }
  const view = replyView(reply);
  return (
    <div className="mt-24 flex flex-col gap-24">
      <TabHeader control={view && <Switch size="sm" label={t("logs.inspector.showRaw", "Show raw")} checked={raw} onCheckedChange={setRaw} />}>
        {view && (
          <Pill size="sm" className="shrink-0">
            {e.error_kind ? t("logs.inspector.incomplete", "Incomplete") : view.stop}
          </Pill>
        )}
        <Text variant="small" tone="secondary">
          {e.stream
            ? e.error_kind
              ? t(
                  "logs.inspector.replyStreamStopped",
                  "Assembled from the stream until it stopped, in Kaeso's own format, which also keeps what the app's format can't show.",
                )
              : t("logs.inspector.replyStream", "Assembled from the stream in Kaeso's own format, which also keeps what the app's format can't show.")
            : t("logs.inspector.replyOwnFormat", "The reply in Kaeso's own format, which also keeps what the app's format can't show.")}
        </Text>
      </TabHeader>
      {view && !raw && !Conversation ? (
        <ConversationPending messages={1} />
      ) : view && !raw && Conversation ? (
        <>
          <Conversation messages={[{ role: "assistant", content: view.blocks }]} />
          {view.unknown.map((u, i) => (
            <div key={i} className="flex flex-col gap-8">
              <Text variant="small" tone="secondary">
                {t("logs.inspector.keptBlock", "A block kept as it came:")}
              </Text>
              <JsonView value={u} defaultDepth={3} />
            </div>
          ))}
        </>
      ) : (
        <JsonView value={reply} defaultDepth={4} />
      )}
    </div>
  );
}

function RawTab({ e, canContent }: { e: LogEntryView; canContent: boolean }) {
  const { content, ...meta } = e;
  return (
    <div className="mt-24 flex flex-col gap-24">
      <TabHeader control={<InlineCopyText value={e.id} label={t("logs.copyRequestId", "Copy request id")} />}>
        <Text variant="small" tone="secondary">
          {t("logs.inspector.rawHint", "The log entry as the API returns it.")}
        </Text>
      </TabHeader>
      <CodeBlock title={t("logs.inspector.metadata", "Metadata")} lang="json" code={JSON.stringify(meta, null, 2)} collapseAfter={0} />
      {canContent && content && (
        <>
          <CodeBlock title={t("logs.inspector.request", "Request")} lang="json" code={JSON.stringify(content.request, null, 2)} />
          <CodeBlock title={t("logs.inspector.reply", "Reply")} lang="json" code={JSON.stringify(content.reply, null, 2)} />
        </>
      )}
    </div>
  );
}
