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
              {...replyView(r)}
              {...(latest && onRetry ? { onRetry } : {})}
              {...(latest ? {} : { footer: <ResponseMetrics {...metricsOf(r, model)} /> })}
            />
          </div>
        );
      })}
    </Turns>
  );
}
