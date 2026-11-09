import { useNeedsRight } from "~/auth/needs-right";
import { useState } from "react";
import { DialogScope, KeyValue, Radio, RadioGroup, StepDialog } from "@logicplanes/kaeso-ui";
import type { LogFilters, ToolCallFilters } from "~/api/queries/logs";
import { startExport } from "~/shell/export-job";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import {
  JSON_LINES,
  exportFilters,
  exportRequest,
  exportSummary,
  toolExportFilters,
  toolExportRequest,
  toolExportSummary,
  type ExportFormat,
  type ExportNames,
  type ToolExportNames,
} from "./export";

interface DialogProps {
  org: string;
  project: string;
  canContent: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

interface ExportDialogProps extends DialogProps {
  filters: LogFilters;
  names: ExportNames;
}

export function ExportDialog({ filters, names, ...props }: ExportDialogProps) {
  return (
    <DialogScope open={props.open}>
      <ExportSteps
        {...props}
        title={t("logs.export.title", "Export requests")}
        summary={exportSummary(filters)}
        applied={exportFilters(filters, names)}
        jsonlHint={t("logs.export.jsonlHint", "Metadata, one request per line.")}
        contentHint={t("logs.export.jsonlContentHint", "Adds the stored request and reply bodies.")}
        progress={(size) => (size ? t("logs.export.progressSize", "Exporting requests · {size}", { size }) : t("logs.export.progress", "Exporting requests"))}
        done={t("logs.export.done", "Requests exported")}
        request={(format) => exportRequest(props.org, props.project, filters, format)}
      />
    </DialogScope>
  );
}

interface ToolCallsExportDialogProps extends DialogProps {
  filters: ToolCallFilters;
  names: ToolExportNames;
  labels: { status?: string; error?: string };
}

export function ToolCallsExportDialog({ filters, names, labels, ...props }: ToolCallsExportDialogProps) {
  return (
    <DialogScope open={props.open}>
      <ExportSteps
        {...props}
        title={t("logs.tools.export.title", "Export tool calls")}
        summary={toolExportSummary(filters)}
        applied={toolExportFilters(filters, names, labels)}
        jsonlHint={t("logs.tools.export.jsonlHint", "Metadata, one tool call per line.")}
        contentHint={t("logs.tools.export.jsonlContentHint", "Adds the stored arguments and results.")}
        progress={(size) =>
          size ? t("logs.tools.export.progressSize", "Exporting tool calls · {size}", { size }) : t("logs.tools.export.progress", "Exporting tool calls")
        }
        done={t("logs.tools.export.done", "Tool calls exported")}
        request={(format) => toolExportRequest(props.org, props.project, filters, format)}
      />
    </DialogScope>
  );
}

interface StepsProps extends DialogProps {
  title: string;
  summary: string;
  applied: { label: string; value: string }[];
  jsonlHint: string;
  contentHint: string;
  progress: (size: string | undefined) => string;
  done: string;
  request: (format: ExportFormat) => ReturnType<typeof exportRequest>;
}

function ExportSteps({ org, project, canContent, open, onOpenChange, title, summary, applied, jsonlHint, contentHint, progress, done, request }: StepsProps) {
  const [format, setFormat] = useState<ExportFormat>("csv");
  const needs = useNeedsRight(org, project);
  return (
    <StepDialog
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      submitLabel={common.export()}
      onSubmit={() => {
        startExport({ title: progress, done, ...request(format) });
        onOpenChange(false);
      }}
      steps={[
        {
          id: "format",
          title: t("logs.export.whichFormat", "Which format?"),
          description: summary,
          content: (
            <div className="flex flex-col gap-16">
              {applied.length > 0 && <KeyValue items={applied} />}
              <RadioGroup variant="cards" aria-label={t("logs.inspector.format", "Format")} value={format} onValueChange={(v) => setFormat(v as ExportFormat)}>
                <Radio value="csv" label="CSV" description={t("logs.export.csvHint", "Metadata, for spreadsheets.")} />
                <Radio value="jsonl" label={JSON_LINES} description={jsonlHint} />
                <Radio
                  value="content"
                  label={t("logs.export.jsonlContent", "JSON Lines with content")}
                  description={canContent ? contentHint : needs("logs.content.view")}
                  disabled={!canContent}
                />
              </RadioGroup>
            </div>
          ),
        },
      ]}
    />
  );
}
