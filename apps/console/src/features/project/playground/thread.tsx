import type { ProjectToolView } from "~/api/generated/types";
import type { PlaygroundModel } from "~/api/queries/playground";
import { Text } from "@logicplanes/kaeso-ui";
import { ResponseMetrics, ResponseView, Thread as Turns, UserMessage } from "@logicplanes/kaeso-ui/playground";
import { metricsOf } from "./metrics";
import { t } from "~/i18n";
import { functionCalls, replyView, type ToolOutput, type Turn } from "./responses";
import { AnsweredCalls, ToolCalls } from "./tool-calls";

export interface ThreadTools {
  org: string;
  project: string;
  list: ProjectToolView[];
  onOutputs: (outputs: ToolOutput[]) => void;
}

export default function Thread({
  turns,
  model,
  label,
  onRetry,
  tools,
}: {
  turns: Turn[];
  model: PlaygroundModel | null;
  label: string;
  onRetry: (() => void) | null;
  tools?: ThreadTools | undefined;
}) {
  const last = turns.at(-1);
  return (
    <Turns>
      {turns.map((turn, i) => {
        const r = turn.response;
        const latest = turn === last;
        const calls = functionCalls(r);
        const answers = turns[i + 1]?.outputs;
        const footer =
          calls.length && answers ? (
            <AnsweredCalls calls={calls} answers={answers} />
          ) : calls.length && latest && tools ? (
            <ToolCalls key={turn.id} org={tools.org} project={tools.project} calls={calls} tools={tools.list} onDone={tools.onOutputs} />
          ) : latest ? null : (
            <ResponseMetrics {...metricsOf(r, model)} />
          );
        return (
          <div key={turn.id} className="flex min-w-0 flex-col gap-12">
            {turn.outputs ? (
              <Text variant="small" tone="secondary">
                {t("playground.tools.sent", "{count, plural, one {Sent the tool's answer} other {Sent the tools' answers}}", { count: turn.outputs.length })}
              </Text>
            ) : (
              <UserMessage>{turn.prompt}</UserMessage>
            )}
            <ResponseView
              aria-label={t("playground.response", "{model} response", { model: label })}
              {...replyView(r)}
              {...(latest && onRetry ? { onRetry } : {})}
              {...(footer ? { footer } : {})}
            />
          </div>
        );
      })}
    </Turns>
  );
}
