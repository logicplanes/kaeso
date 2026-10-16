import { useNeedsRight } from "~/auth/needs-right";
import { useState } from "react";
import { KeyValue, Radio, RadioCards, StepDialog } from "@logicplanes/kaeso-ui";
import type { LogFilters } from "~/api/queries/logs";
import { startExport } from "~/shell/export-job";
import { t } from "~/i18n";
import { common } from "~/i18n/common";
import { JSON_LINES, exportFilters, exportRequest, exportSummary, type ExportFormat, type ExportNames } from "./export";

export function ExportDialog({
  org,
  project,
  filters,
  names,
  canContent,
  open,
  onOpenChange,
}: {
  org: string;
  project: string;
  filters: LogFilters;
  names: ExportNames;
  canContent: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [format, setFormat] = useState<ExportFormat>("csv");
  const needs = useNeedsRight(org, project);
  const applied = exportFilters(filters, names);
  return (
    <StepDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("logs.export.title", "Export requests")}
      submitLabel={common.export()}
      onSubmit={() => {
        startExport({
          title: (size) => (size ? t("logs.export.progressSize", "Exporting requests · {size}", { size }) : t("logs.export.progress", "Exporting requests")),
          done: t("logs.export.done", "Requests exported"),
          ...exportRequest(org, project, filters, format),
        });
        onOpenChange(false);
      }}
      steps={[
        {
          id: "format",
          title: t("logs.export.whichFormat", "Which format?"),
          description: exportSummary(filters),
          content: (
            <div className="flex flex-col gap-16">
              {applied.length > 0 && <KeyValue items={applied} />}
              <RadioCards aria-label={t("logs.inspector.format", "Format")} value={format} onValueChange={(v) => setFormat(v as ExportFormat)}>
                <Radio value="csv" label="CSV" description={t("logs.export.csvHint", "Metadata, for spreadsheets.")} />
                <Radio value="jsonl" label={JSON_LINES} description={t("logs.export.jsonlHint", "Metadata, one request per line.")} />
                <Radio
                  value="content"
                  label={t("logs.export.jsonlContent", "JSON Lines with content")}
                  description={canContent ? t("logs.export.jsonlContentHint", "Adds the stored request and reply bodies.") : needs("logs.content.view")}
                  disabled={!canContent}
                />
              </RadioCards>
            </div>
          ),
        },
      ]}
    />
  );
}
