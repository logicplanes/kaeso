import type { PlaygroundModel } from "~/api/queries/playground";
import { ResponseMetrics, ResponseView, Thread as Turns, UserMessage } from "@logicplanes/kaeso-ui/playground";
import { metricsOf } from "./metrics";
import { t } from "~/i18n";
import { replyView, type Turn } from "./responses";

export default function Thread({
  turns,
  model,
  label,
  onRetry,
  className,
}: {
  turns: Turn[];
  model: PlaygroundModel | null;
  label: string;
  onRetry: (() => void) | null;
  className?: string | undefined;
}) {
  const last = turns.at(-1);
  return (
    <Turns className={className}>
      {turns.map((turn) => {
        const r = turn.response;
        const metrics = r.status === "failed" && !r.usage ? null : <ResponseMetrics {...metricsOf(r, model)} />;
        return (
          <div key={turn.id} className="flex min-w-0 flex-col gap-12">
            <UserMessage>{turn.prompt}</UserMessage>
            <ResponseView
              aria-label={t("playground.response", "{model} response", { model: label })}
              {...replyView(r)}
              {...(turn === last && onRetry ? { onRetry } : {})}
              {...(metrics ? { footer: metrics } : {})}
            />
          </div>
        );
      })}
    </Turns>
  );
}
