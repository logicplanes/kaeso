import { Empty } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export function NotLogged({ observed }: { observed?: boolean }) {
  return observed ? (
    <Empty
      title={t("logs.inspector.notKept", "Content not kept")}
      description={t("logs.inspector.notKeptHint", "The source keeps no content, its client sent none, or its retention has passed.")}
    />
  ) : (
    <Empty
      title={t("logs.inspector.notLogged", "Content not logged")}
      description={t("logs.inspector.notLoggedHint", "Logging was off when this request ran, or its content has passed the retention period.")}
    />
  );
}
