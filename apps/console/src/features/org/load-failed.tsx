import { Empty } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export function LoadFailed({ title, size }: { title: string; size?: "md" | undefined }) {
  return <Empty {...(size ? { size } : {})} title={title} description={t("org.loadFailed.hint", "The notification says why; try again from there.")} />;
}
