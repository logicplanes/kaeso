import { memo } from "react";
import type { PlaygroundModel } from "~/api/queries/playground";
import { MediaResponse, ResponseMetrics, ResponseView, Thread as Turns, UserMessage, type MediaKind, type MediaStatus } from "@logicplanes/kaeso-ui/playground";
import { metricsOf } from "./metrics";
import { t } from "~/i18n";
import { shown } from "./attachments";
import { replyView, type Mode, type ResponseState, type Turn } from "./responses";

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

const errorOf = (r: ResponseState) =>
  r.error ? (r.requestId ? t("playground.errorWithRequest", "{error} Request {id}", { error: r.error, id: r.requestId }) : r.error) : undefined;

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
}: {
  turn: Turn;
  model: PlaygroundModel | null;
  label: string;
  threadKey: string;
  expected: number;
  onRetry: ((thread: string) => void) | null;
  onCancel: ((turn: string) => void) | null;
}) {
  const r = turn.response;
  const mode = turn.mode ?? "text";
  const kind = KIND[mode];
  const metrics = r.status === "failed" && !r.usage ? null : <ResponseMetrics {...metricsOf(r, model, mode)} />;
  const name = t("playground.response", "{model} response", { model: label });
  const error = errorOf(r);
  return (
    <div className="flex min-w-0 flex-col gap-12">
      <UserMessage attachments={shown(turn.files)}>{turn.prompt}</UserMessage>
      {kind ? (
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
          {...replyView(r)}
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
  className,
}: {
  turns: Turn[];
  model: PlaygroundModel | null;
  label: string;
  threadKey: string;
  expected?: number | undefined;
  onRetry: ((thread: string) => void) | null;
  onCancel?: ((turn: string) => void) | null | undefined;
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
        />
      ))}
    </Turns>
  );
}
