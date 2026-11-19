import { Button, Dialog, DiffView } from "@logicplanes/kaeso-ui";
import { CodeBlock } from "@logicplanes/kaeso-ui/code-block";
import type { RouteStep } from "~/api/generated/types";
import { stableJson } from "~/format/stable-json";
import { t } from "~/i18n";
import { DEFAULT_FALL_BACK_ON } from "./draft";

export function modelRouteOf(model: string): RouteStep {
  return { type: "model", model, fallback: null, fall_back_on: [...DEFAULT_FALL_BACK_ON], timeout_seconds: null, retries: null };
}

export function RouteJsonDialog({ open, onOpenChange, name, route }: { open: boolean; onOpenChange: (open: boolean) => void; name: string; route: RouteStep }) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="lg"
      title={t("models.route.jsonTitle", "The route as JSON")}
      description={t("models.route.jsonHint", "Send this as the alias's route on the Platform API to set it from a script.")}
      footer={
        <Dialog.Close>
          <Button>{t("common.close", "Close")}</Button>
        </Dialog.Close>
      }
    >
      <CodeBlock lang="json" title={name || t("models.route.newAlias", "New alias")} code={JSON.stringify({ name, route }, null, 2)} collapseAfter={0} />
    </Dialog>
  );
}

type Named = { name: string; route: RouteStep };

/** The saved alias against the draft, serialised the same way, so only what changed differs. */
export function RouteChangesDialog({ open, onOpenChange, saved, draft }: { open: boolean; onOpenChange: (open: boolean) => void; saved: Named; draft: Named }) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="lg"
      title={t("models.route.changesTitle", "Unsaved changes")}
      description={t("models.route.changesHint", "The saved alias, and what saving would make it; requests take the new route once you save.")}
      footer={
        <Dialog.Close>
          <Button>{t("common.close", "Close")}</Button>
        </Dialog.Close>
      }
    >
      <DiffView title={t("models.route.savedToDraft", "Saved → draft")} lang="json" before={stableJson(saved)} after={stableJson(draft)} wrap />
    </Dialog>
  );
}
