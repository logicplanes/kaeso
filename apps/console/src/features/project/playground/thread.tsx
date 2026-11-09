import { memo } from "react";
import type { PlaygroundModel } from "~/api/queries/playground";
import { ResponseMetrics, ResponseView, Thread as Turns, UserMessage } from "@logicplanes/kaeso-ui/playground";
import { metricsOf } from "./metrics";
import { t } from "~/i18n";
import { shown } from "./attachments";
import { replyView, type Turn } from "./responses";

// A turn that finished keeps its object while the newest one streams, so only the newest renders
// again on each frame of a reply.
const TurnView = memo(function TurnView({
  turn,
  model,
  label,
  threadKey,
  onRetry,
}: {
  turn: Turn;
  model: PlaygroundModel | null;
  label: string;
  threadKey: string;
  onRetry: ((thread: string) => void) | null;
}) {
  const r = turn.response;
  const metrics = r.status === "failed" && !r.usage ? null : <ResponseMetrics {...metricsOf(r, model)} />;
  return (
    <div className="flex min-w-0 flex-col gap-12">
      <UserMessage attachments={shown(turn.files)}>{turn.prompt}</UserMessage>
      <ResponseView
        aria-label={t("playground.response", "{model} response", { model: label })}
        {...replyView(r)}
        {...(onRetry ? { onRetry: () => onRetry(threadKey) } : {})}
        {...(metrics ? { footer: metrics } : {})}
      />
    </div>
  );
});

export default function Thread({
  turns,
  model,
  label,
  threadKey,
  onRetry,
  className,
}: {
  turns: Turn[];
  model: PlaygroundModel | null;
  label: string;
  threadKey: string;
  onRetry: ((thread: string) => void) | null;
  className?: string | undefined;
}) {
  const last = turns.at(-1);
  return (
    <Turns className={className}>
      {turns.map((turn) => (
        <TurnView key={turn.id} turn={turn} model={model} label={label} threadKey={threadKey} onRetry={turn === last ? onRetry : null} />
      ))}
    </Turns>
  );
}
