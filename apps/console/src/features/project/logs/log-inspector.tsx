import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Badge,
  Button,
  CodeBlock,
  Collapsible,
  Empty,
  CopyButton,
  JsonView,
  KeyValue,
  Tag,
  ProviderBadge,
  RequestTimeline,
  Section,
  Sheet,
  Skeleton,
  StatGroup,
  Stat,
  StatusCode,
  Switch,
  Tabs,
  Text,
  Tooltip,
  UsageBreakdown,
} from "@logicplanes/kaeso-ui";
import { formatBytes, formatDate, formatNumber, formatPercent } from "@logicplanes/kaeso-ui/format";
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
import { cachedShare, isEmbedding, sentWithoutBreakpoint } from "./columns";
import { mediaCount, mediaLabel } from "./media";
import { decisionAnswers, decisionView, embeddingsView, isKnownReplyForm, replyView, requestView, type DecisionView } from "./render";
import { collectMedia, forReading } from "./logged-media";
import { MediaList } from "./kept-media";
import { microseconds, seconds, startedLabel } from "./timing";
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
  return mod;
}

function ConversationPending({ messages }: { messages: number }) {
  return (
    <div className="flex flex-col gap-24" aria-busy="true">
      {Array.from({ length: Math.min(messages, 4) }, (_, i) => (
        <div key={i} className="flex flex-col gap-8">
          <Skeleton shape="line" className="w-64" />
          <Skeleton lines={3} />
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
  const loaded = useConversation();
  const conversation = loaded?.default;
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
      <Tabs.Panel value="request">
        {canContent ? <RequestTab e={e} Conversation={conversation} Decision={loaded} /> : <NoRight org={org} project={project} />}
      </Tabs.Panel>
      <Tabs.Panel value="reply">
        {canContent ? <ReplyTab e={e} Conversation={conversation} Answers={loaded?.DecisionAnswers} /> : <NoRight org={org} project={project} />}
      </Tabs.Panel>
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
  const embeddings = isEmbedding(e);
  const share = embeddings ? null : cachedShare(u);
  const unmarked = sentWithoutBreakpoint(e);
  const caching = [
    ...(share !== null ? [{ label: t("logs.inspector.cachedShare", "Read from the cache"), value: formatPercent(share) }] : []),
    ...(known(e.cache_breakpoints) ? [{ label: t("logs.inspector.cacheBreakpoints", "Cache breakpoints"), value: formatNumber(e.cache_breakpoints) }] : []),
    ...(e.response_cache ? [{ label: t("logs.inspector.responseCache", "Response cache"), value: responseCacheLabel(e.response_cache) }] : []),
  ];
  const answer = Math.max(0, u.output_tokens - u.reasoning_tokens);
  const timing = [
    ...(known(e.request_upload_us) ? [{ label: t("logs.inspector.requestUpload", "Request upload"), value: microseconds(e.request_upload_us) }] : []),
    ...(known(e.overhead_us) ? [{ label: t("logs.inspector.overhead", "Kaeso overhead"), value: microseconds(e.overhead_us) }] : []),
    ...(sp && known(sp.processing_us) ? [{ label: t("logs.inspector.streamProcessing", "Stream processing"), value: microseconds(sp.processing_us) }] : []),
  ];
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
      <StatGroup aria-label={t("logs.inspector.summary", "Summary")}>
        <Stat
          variant="tile"
          label={t("logs.column.duration", "Duration")}
          value={seconds(e.duration_ms)}
          detail={
            <Tooltip content={formatDate(e.started_at, { time: true, seconds: true })}>
              <time dateTime={e.started_at}>{startedLabel(e.started_at)}</time>
            </Tooltip>
          }
        />
        <Stat
          variant="tile"
          label={t("logs.column.firstByte", "First byte")}
          value={known(e.ttfb_ms) ? seconds(e.ttfb_ms) : "—"}
          detail={known(e.ttfb_ms) ? t("logs.inspector.fromProvider", "From the provider") : t("logs.inspector.noneArrived", "None arrived")}
        />
        <Stat
          variant="tile"
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
        <Stat
          variant="tile"
          label={common.cost()}
          value={e.cost_known ? costUSD(e.cost) : t("kaeso.usage.costUnknown", "Unknown")}
          detail={
            !e.cost_known
              ? t("logs.inspector.costUnknown", "No price for what it made")
              : e.cost_basis === "measured"
                ? t("logs.inspector.costMeasured", "From the audio's length, which Kaeso read from the file")
                : u.estimated
                  ? t("logs.inspector.estimated", "Estimated")
                  : undefined
          }
        />
      </StatGroup>
      {(known(e.ttfb_ms) || timing.length > 0) && (
        <Section title={t("logs.inspector.timing", "Timing")} spacing="none">
          <div className="flex flex-col gap-16">
            {known(e.ttfb_ms) && <RequestTimeline ttfbMs={e.ttfb_ms} totalMs={e.duration_ms} className="w-full" />}
            {timing.length > 0 && <KeyValue layout="stacked" className="-my-8 grid grid-cols-2 gap-x-32" items={timing} />}
          </div>
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
            ...(e.media
              ? [
                  {
                    label: mediaLabel(e.media.unit),
                    value: (
                      <span className="flex min-w-0 flex-col">
                        <Text>{mediaCount(e.media)}</Text>
                        {e.media.detail && (
                          <Text variant="small" tone="secondary">
                            {e.media.detail}
                          </Text>
                        )}
                      </span>
                    ),
                  },
                ]
              : []),
            ...(e.alias ? [{ label: t("logs.inspector.alias", "Alias"), value: <Long mono>{e.alias}</Long> }] : []),
            ...(e.fallbacks.length > 0
              ? [
                  {
                    label: t("logs.inspector.triedFirst", "Tried first"),
                    value: (
                      <span className="flex min-w-0 flex-col gap-4">
                        {e.fallbacks.map((f, i) => (
                          <span key={`${i}-${f.catalogue_model_id}`} className="flex min-w-0 flex-col">
                            <span className="flex min-w-0 items-center gap-8">
                              <StatusCode code={f.status} />
                              <Long mono>{f.model}</Long>
                            </span>
                            <Text variant="small" tone="secondary">
                              {f.skipped
                                ? t("logs.inspector.skippedRecently", "Skipped; it failed moments ago")
                                : t("logs.inspector.failedAfter", "Failed after {duration}", { duration: seconds(f.duration_ms) })}
                            </Text>
                          </span>
                        ))}
                      </span>
                    ),
                  },
                ]
              : []),
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
          actions={
            u.estimated || unmarked ? (
              <div className="flex gap-8">
                {unmarked && <Badge tone="warning">{t("logs.inspector.noBreakpoint", "No cache breakpoint")}</Badge>}
                {u.estimated && <Badge tone="warning">{t("logs.inspector.estimated", "Estimated")}</Badge>}
              </div>
            ) : undefined
          }
        >
          <div className="flex flex-col gap-16">
            {u.estimated && (
              <Text variant="small" tone="secondary">
                {t("logs.inspector.estimatedHint", "The stream ended before the provider's final count; cost and limits use these numbers.")}
              </Text>
            )}
            {unmarked && (
              <Text variant="small" tone="secondary">
                <CodeText
                  text={t(
                    "logs.inspector.noBreakpointHint",
                    "Sent without `cache_control`, and nothing was read from or written to the cache; Anthropic caches only the prompt a request marks.",
                  )}
                />
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
            {caching.length > 0 && <KeyValue layout="stacked" className="-my-8 grid grid-cols-2 gap-x-32" items={caching} />}
          </div>
        </Section>
      )}
    </div>
  );
}

function TabHeader({ children, control }: { children: ReactNode; control?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-16 gap-y-12">
      <div className="flex min-w-0 flex-1 basis-280 items-baseline gap-8">{children}</div>
      {control && <div className="flex shrink-0 items-center">{control}</div>}
    </div>
  );
}

type ConversationView = ConversationModule["default"] | undefined;

function RequestTab({ e, Conversation, Decision }: { e: LogEntryView; Conversation: ConversationView; Decision: ConversationModule | undefined }) {
  const [raw, setRaw] = useState(false);
  const body = e.content?.request as unknown;
  // Kept media is megabytes of base64: reading it again on every render would decode it anew and restart players.
  const view = useMemo(() => requestView(e.ingress_dialect, body), [e.ingress_dialect, body]);
  const media = useMemo(() => collectMedia(body), [body]);
  const readable = useMemo(() => forReading(body), [body]);
  if (!e.content) return <NotLogged />;
  const embeddings = embeddingsView(e.ingress_dialect, body);
  const decision = decisionView(e.ingress_dialect, body);
  const mapped = !!(view || embeddings || decision);
  return (
    <div className="mt-24 flex flex-col gap-24">
      <TabHeader control={mapped && <Switch size="sm" label={t("logs.inspector.showRaw", "Show raw")} checked={raw} onCheckedChange={setRaw} />}>
        <Text variant="small" tone="secondary">
          {t("logs.inspector.requestAsSent", "The app's body as sent, in {format}.", { format: dialectLabel(e.ingress_dialect) })}
        </Text>
      </TabHeader>
      {raw || !mapped ? (
        <>
          {!mapped && <MediaList media={media} />}
          <JsonView value={readable} defaultDepth={4} />
        </>
      ) : decision ? (
        <DecisionRequest decision={decision} parts={Decision} />
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

type AnswersView = ConversationModule["DecisionAnswers"] | undefined;

function DecisionRequest({ decision, parts }: { decision: DecisionView; parts: ConversationModule | undefined }) {
  const state = useMemo(() => forReading(decision.state), [decision.state]);
  return (
    <>
      <Section title={t("logs.decision.state", "What the questions are about")} spacing="none">
        {typeof decision.state !== "string" ? (
          <JsonView value={state} defaultDepth={3} />
        ) : parts ? (
          <parts.DecisionState text={decision.state} />
        ) : (
          <ConversationPending messages={1} />
        )}
      </Section>
      <Section title={t("logs.decision.questions", "Questions")} spacing="none">
        {parts ? <parts.DecisionQuestions questions={decision.questions} /> : <ConversationPending messages={decision.questions.length} />}
      </Section>
    </>
  );
}

function DecisionReply({
  e,
  decided,
  readable,
  Answers,
}: {
  e: LogEntryView;
  decided: NonNullable<ReturnType<typeof decisionAnswers>>;
  readable: unknown;
  Answers: AnswersView;
}) {
  const [raw, setRaw] = useState(false);
  const asked = useMemo(() => decisionView(e.ingress_dialect, e.content?.request as unknown), [e.ingress_dialect, e.content?.request]);
  const questions = asked?.questions ?? decided.answers.map((a) => ({ kind: a.kind === "refusal" ? ("predicate" as const) : a.kind }));
  return (
    <div className="mt-24 flex flex-col gap-24">
      <TabHeader control={<Switch size="sm" label={t("logs.inspector.showRaw", "Show raw")} checked={raw} onCheckedChange={setRaw} />}>
        <Text variant="small" tone="secondary">
          {t("logs.decision.replyHint", "Each answer with its probabilities, in the order the questions were asked.")}
        </Text>
      </TabHeader>
      {raw ? (
        <JsonView value={readable} defaultDepth={4} />
      ) : Answers ? (
        <Answers questions={questions} answers={decided.answers} />
      ) : (
        <ConversationPending messages={questions.length} />
      )}
    </div>
  );
}

function ReplyTab({ e, Conversation, Answers }: { e: LogEntryView; Conversation: ConversationView; Answers: AnswersView }) {
  const [raw, setRaw] = useState(false);
  const reply = e.content?.reply as unknown;
  const view = useMemo(() => replyView(reply), [reply]);
  const media = useMemo(() => collectMedia(reply), [reply]);
  const readable = useMemo(() => forReading(reply), [reply]);
  if (!e.content) return <NotLogged />;
  if (isEmbedding(e)) {
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
  const decided = decisionAnswers(reply);
  if (decided) return <DecisionReply e={e} decided={decided} readable={readable} Answers={Answers} />;
  if (!isKnownReplyForm(reply)) {
    return (
      <div className="mt-24 flex flex-col gap-24">
        <TabHeader>
          <Text variant="small" tone="secondary">
            {t("logs.inspector.replyUnknownFormat", "This reply was logged in a format this console can't read yet; it's shown as stored.")}
          </Text>
        </TabHeader>
        <JsonView value={readable} defaultDepth={4} />
      </div>
    );
  }
  return (
    <div className="mt-24 flex flex-col gap-24">
      <TabHeader control={view && <Switch size="sm" label={t("logs.inspector.showRaw", "Show raw")} checked={raw} onCheckedChange={setRaw} />}>
        {view && (
          <Tag size="sm" className="shrink-0">
            {e.error_kind ? t("logs.inspector.incomplete", "Incomplete") : view.stop}
          </Tag>
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
        <>
          {!view && <MediaList media={media} />}
          <JsonView value={readable} defaultDepth={4} />
        </>
      )}
    </div>
  );
}

function RawTab({ e, canContent }: { e: LogEntryView; canContent: boolean }) {
  const { content, ...meta } = e;
  const request = useMemo(() => JSON.stringify(forReading(content?.request), null, 2), [content?.request]);
  const reply = useMemo(() => JSON.stringify(forReading(content?.reply), null, 2), [content?.reply]);
  return (
    <div className="mt-24 flex flex-col gap-24">
      <TabHeader control={<CopyButton display="inline" value={e.id} label={t("logs.copyRequestId", "Copy request id")} />}>
        <Text variant="small" tone="secondary">
          {t("logs.inspector.rawHint", "The log entry as the API returns it.")}
        </Text>
      </TabHeader>
      <CodeBlock title={t("logs.inspector.metadata", "Metadata")} lang="json" code={JSON.stringify(meta, null, 2)} collapseAfter={0} />
      {canContent && content && (
        <>
          <CodeBlock title={t("logs.inspector.request", "Request")} lang="json" code={request} />
          <CodeBlock title={t("logs.inspector.reply", "Reply")} lang="json" code={reply} />
        </>
      )}
    </div>
  );
}

function responseCacheLabel(result: NonNullable<LogEntryView["response_cache"]>): string {
  switch (result) {
    case "hit":
      return t("logs.inspector.responseCacheHit", "Answered from the cache, at no provider cost");
    case "miss":
      return t("logs.inspector.responseCacheMiss", "Not in the cache; sent to the provider");
    case "bypass":
      return t("logs.inspector.responseCacheBypass", "Not cacheable, or skipped by the app");
  }
}
