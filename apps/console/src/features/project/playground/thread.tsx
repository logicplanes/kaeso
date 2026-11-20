import { memo } from "react";
import { Button, Link, icons } from "@logicplanes/kaeso-ui";
import type { PlaygroundModel } from "~/api/queries/playground";
import {
  DecisionResponse,
  MediaResponse,
  ResponseMetrics,
  ResponseView,
  Thread as Turns,
  UserMessage,
  type DecisionImage,
  type DecisionStatus,
  type MediaKind,
  type MediaStatus,
} from "@logicplanes/kaeso-ui/playground";
import { metricsOf } from "./metrics";
import { t } from "~/i18n";
import { shown } from "./attachments";
import { isLive, replyView, type Mode, type ResponseState, type Turn } from "./responses";

const KIND: Partial<Record<Mode, MediaKind>> = { image: "image", speech: "audio", sound_effects: "audio", music: "audio", video: "video" };

function mediaStatus(r: ResponseState): MediaStatus {
  switch (r.status) {
    case "waiting":
      return "pending";
    case "streaming":
      return "running";
    case "failed":
      return "error";
    case "stopped":
      return "stopped";
    default:
      return "done";
  }
}

function decisionStatus(r: ResponseState): DecisionStatus {
  switch (r.status) {
    case "waiting":
    case "streaming":
      return "pending";
    case "failed":
    case "stopped":
      return "error";
    default:
      return "done";
  }
}

const imageOf = (turn: Turn): DecisionImage | undefined => {
  const file = turn.files?.find((f) => f.type.startsWith("image/"));
  return file ? { src: file.data, alt: file.name } : undefined;
};

// Someone who can open the request log gets a link instead of the id in the message.
const errorOf = (r: ResponseState, linked: boolean) =>
  r.error ? (r.requestId && !linked ? t("playground.errorWithRequest", "{error} Request {id}", { error: r.error, id: r.requestId }) : r.error) : undefined;

// A turn that finished keeps its object while the newest one streams, so only the newest renders
// again on each frame of a reply.
const TurnView = memo(function TurnView({
  turn,
  model,
  label,
  threadKey,
  expected,
  onRetry,
  onCancel,
  onEdit,
  logHref,
  formatCost,
}: {
  turn: Turn;
  model: PlaygroundModel | null;
  label: string;
  threadKey: string;
  expected: number;
  onRetry: ((thread: string) => void) | null;
  onCancel: ((turn: string) => void) | null;
  onEdit: (() => void) | null;
  logHref: ((id: string) => string) | null;
  formatCost: ((usd: string) => string) | undefined;
}) {
  const r = turn.response;
  const mode = turn.mode ?? "text";
  const kind = KIND[mode];
  // A reply reaches the log once it ended, so the link waits for that.
  const log = logHref && r.requestId && !isLive(r) ? logHref(r.requestId) : null;
  const shownMetrics = r.status === "failed" && !r.usage ? null : <ResponseMetrics {...metricsOf(r, model, mode)} formatCost={formatCost} />;
  const metrics =
    shownMetrics || log ? (
      <span className="flex flex-wrap items-center gap-x-16 gap-y-4">
        {shownMetrics}
        {log && (
          <Link href={log} variant="quiet">
            {t("playground.viewInLog", "View in request log")}
          </Link>
        )}
      </span>
    ) : null;
  const name = t("playground.response", "{model} response", { model: label });
  const error = errorOf(r, !!logHref);
  return (
    <div className="flex min-w-0 flex-col gap-12">
      <UserMessage
        attachments={shown(turn.files)}
        {...(onEdit
          ? {
              actions: (
                <Button size="sm" variant="quiet" leadingIcon={icons.edit} onClick={onEdit}>
                  {t("playground.edit", "Edit")}
                </Button>
              ),
            }
          : {})}
      >
        {turn.prompt}
      </UserMessage>
      {mode === "decision" ? (
        <DecisionResponse
          aria-label={name}
          questions={turn.questions ?? []}
          answers={r.answers}
          status={decisionStatus(r)}
          image={imageOf(turn)}
          {...(r.status === "stopped" ? { error: t("playground.decisions.stopped", "Stopped before the model answered.") } : error ? { error } : {})}
          {...(onRetry ? { onRetry: () => onRetry(threadKey) } : {})}
          {...(metrics ? { footer: metrics } : {})}
        />
      ) : kind ? (
        <MediaResponse
          aria-label={name}
          kind={kind}
          status={mediaStatus(r)}
          items={r.media ?? []}
          expected={expected}
          progress={r.progress}
          {...(error ? { error } : {})}
          {...(onCancel && kind === "video" ? { onCancel: () => onCancel(turn.id) } : {})}
          {...(onRetry ? { onRetry: () => onRetry(threadKey) } : {})}
          {...(metrics ? { footer: metrics } : {})}
        />
      ) : (
        <ResponseView
          aria-label={name}
          {...replyView(r, !!logHref)}
          {...(onRetry ? { onRetry: () => onRetry(threadKey) } : {})}
          {...(metrics ? { footer: metrics } : {})}
        />
      )}
    </div>
  );
});

export default function Thread({
  turns,
  model,
  label,
  threadKey,
  expected = 1,
  onRetry,
  onCancel = null,
  onEdit = null,
  logHref = null,
  formatCost,
  className,
}: {
  turns: Turn[];
  model: PlaygroundModel | null;
  label: string;
  threadKey: string;
  expected?: number | undefined;
  onRetry: ((thread: string) => void) | null;
  onCancel?: ((turn: string) => void) | null | undefined;
  /** Takes the last prompt back into the composer, from every column. */
  onEdit?: (() => void) | null | undefined;
  /** Where a reply's entry in the request log opens, for someone who may read the log. */
  logHref?: ((id: string) => string) | null | undefined;
  /** Writes a reply's cost, given in dollars, such as in the organization's currency. */
  formatCost?: ((usd: string) => string) | undefined;
  className?: string | undefined;
}) {
  const last = turns.at(-1);
  return (
    <Turns className={className}>
      {turns.map((turn) => (
        <TurnView
          key={turn.id}
          turn={turn}
          model={model}
          label={label}
          threadKey={threadKey}
          expected={expected}
          onRetry={turn === last ? onRetry : null}
          onCancel={turn === last ? onCancel : null}
          onEdit={turn === last ? onEdit : null}
          logHref={logHref}
          formatCost={formatCost}
        />
      ))}
    </Turns>
  );
}
