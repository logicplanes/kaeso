import type { PlaygroundModel } from "~/api/queries/playground";
import { ResponseMetrics, ResponseView, Thread as Turns, UserMessage, type ResponseStatus } from "@logicplanes/kaeso-ui/playground";
import { metricsOf } from "./metrics";
import { t } from "~/i18n";
import type { ResponseState, Turn } from "./responses";

export default function Thread({
  turns,
  model,
  label,
  onRetry,
}: {
  turns: Turn[];
  model: PlaygroundModel | null;
  label: string;
  onRetry: (() => void) | null;
}) {
  const last = turns.at(-1);
  return (
    <Turns>
      {turns.map((turn) => {
        const r = turn.response;
        const latest = turn === last;
        return (
          <div key={turn.id} className="flex min-w-0 flex-col gap-12">
            <UserMessage>{turn.prompt}</UserMessage>
            <ResponseView
              aria-label={t("playground.response", "{model} response", { model: label })}
              text={r.text}
              reasoning={r.reasoning}
              status={statusOf(r)}
              {...(r.incompleteReason ? { incompleteReason: r.incompleteReason } : {})}
              {...(r.error
                ? { error: r.requestId ? t("playground.errorWithRequest", "{error} Request {id}", { error: r.error, id: r.requestId }) : r.error }
                : {})}
              {...(r.reasoning && r.firstTextAt !== null && r.firstTokenAt !== null ? { reasoningDuration: r.firstTextAt - r.firstTokenAt } : {})}
              {...(latest && onRetry ? { onRetry } : {})}
              {...(latest ? {} : { footer: <ResponseMetrics {...metricsOf(r, model)} /> })}
            />
          </div>
        );
      })}
    </Turns>
  );
}

const statusOf = (r: ResponseState): ResponseStatus =>
  r.status === "waiting" ? "pending" : r.status === "completed" ? "done" : r.status === "failed" ? "error" : r.status;
