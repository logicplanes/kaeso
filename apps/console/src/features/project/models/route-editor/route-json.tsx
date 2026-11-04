import { Button, Dialog } from "@logicplanes/kaeso-ui";
import { CodeBlock } from "@logicplanes/kaeso-ui/code-block";
import type { RouteStep } from "~/api/generated/types";
import { t } from "~/i18n";
import { DEFAULT_FALL_BACK_ON } from "./draft";

export function modelRouteOf(model: string): RouteStep {
  return { type: "model", model, fallback: null, fall_back_on: [...DEFAULT_FALL_BACK_ON], timeout_seconds: null };
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
