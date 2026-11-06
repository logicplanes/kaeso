import { useEffect, useRef, useState } from "react";
import { Button, Code, JsonView, Status, Text } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import type { ProjectToolView } from "~/api/generated/types";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";
import type { FunctionCall, ToolOutput } from "./responses";

type Decision = { state: "running" } | { state: "done"; output: string; failed: boolean } | { state: "skipped"; output: string };

function parsed(args: string): unknown {
  try {
    return JSON.parse(args) as unknown;
  } catch {
    return args;
  }
}

type Answer = { result?: Record<string, unknown> | null; error?: Record<string, unknown> | null };

// What the model reads back: the tool's text content when it has some, its whole result otherwise.
export function outputOf(answer: Answer): { output: string; failed: boolean } {
  if (answer.error) return { output: String(answer.error.message ?? JSON.stringify(answer.error)), failed: true };
  const result = answer.result ?? {};
  const content = Array.isArray(result.content) ? (result.content as Record<string, unknown>[]) : [];
  const text = content
    .filter((c) => c.type === "text")
    .map((c) => String(c.text ?? ""))
    .join("\n\n");
  return { output: text || JSON.stringify(result), failed: result.isError === true };
}

export function ToolCalls({
  org,
  project,
  calls,
  tools,
  onDone,
}: {
  org: string;
  project: string;
  calls: FunctionCall[];
  tools: ProjectToolView[];
  onDone: (outputs: ToolOutput[]) => void;
}) {
  const [decisions, setDecisions] = useState<Record<string, Decision>>({});
  const decide = (call: FunctionCall, d: Decision) => setDecisions((all) => ({ ...all, [call.call_id]: d }));
  const outputs = calls.flatMap((c) => {
    const d = decisions[c.call_id];
    return d && d.state !== "running" ? [{ call_id: c.call_id, output: d.output }] : [];
  });
  const complete = outputs.length === calls.length;
  const sent = useRef(false);
  const finish = useRef(onDone);
  useEffect(() => {
    finish.current = onDone;
  });
  useEffect(() => {
    if (!complete || sent.current) return;
    sent.current = true;
    finish.current(outputs);
  }, [complete, outputs]);
  const run = async (call: FunctionCall) => {
    decide(call, { state: "running" });
    const tool = tools.find((x) => x.exposed_name === call.name);
    if (!tool) {
      decide(call, { state: "done", output: t("playground.tools.unknown", "This project has no tool {name}.", { name: call.name }), failed: true });
      return;
    }
    try {
      const answer = data(
        await api.POST("/platform/v1/orgs/{org}/projects/{project}/tools/{tool}/call", {
          params: { path: { org, project, tool: tool.id } },
          body: { arguments: parsed(call.arguments) as Record<string, never> },
        }),
      );
      decide(call, { state: "done", ...outputOf(answer as Answer) });
    } catch (e) {
      decide(call, { state: "done", output: (e as Error).message, failed: true });
    }
  };
  const skip = (call: FunctionCall) =>
    decide(call, { state: "skipped", output: t("playground.tools.skippedOutput", "The person chose not to run this tool.") });
  return (
    <div className="flex flex-col gap-16">
      {calls.map((call) => {
        const d = decisions[call.call_id];
        return (
          <div key={call.call_id} className="flex flex-col gap-8">
            <Text variant="small" tone="secondary">
              {tRich("playground.tools.wants", "The model wants to call {tool}.", { tool: <Code>{call.name}</Code> })}
            </Text>
            <JsonView value={parsed(call.arguments)} defaultDepth={2} />
            {!d || d.state === "running" ? (
              <span className="flex gap-8">
                <Button size="sm" variant="primary" loading={d?.state === "running"} onClick={() => void run(call)}>
                  {t("playground.tools.run", "Run")}
                </Button>
                <Button size="sm" disabled={d?.state === "running"} onClick={() => skip(call)}>
                  {t("playground.tools.skip", "Skip")}
                </Button>
              </span>
            ) : d.state === "skipped" ? (
              <Status tone="neutral">{t("playground.tools.skipped", "Skipped")}</Status>
            ) : (
              <div className="flex flex-col gap-4">
                <Status tone={d.failed ? "danger" : "success"}>{d.failed ? t("playground.tools.error", "Failed") : t("playground.tools.ran", "Ran")}</Status>
                <Text variant="small">{d.output}</Text>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

export function AnsweredCalls({ calls, answers }: { calls: FunctionCall[]; answers: ToolOutput[] }) {
  return (
    <div className="flex flex-col gap-12">
      {calls.map((call) => (
        <div key={call.call_id} className="flex flex-col gap-4">
          <Text variant="small" tone="secondary">
            {tRich("playground.tools.called", "Called {tool}.", { tool: <Code>{call.name}</Code> })}
          </Text>
          <Text variant="small">{answers.find((a) => a.call_id === call.call_id)?.output}</Text>
        </div>
      ))}
    </div>
  );
}
