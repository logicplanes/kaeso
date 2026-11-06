import { Input } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export function HeaderFields({ name, value, onName, onValue }: { name: string; value: string; onName: (v: string) => void; onValue: (v: string) => void }) {
  return (
    <div className="flex flex-col gap-16">
      <Input label={t("orgHome.mcp.headerName", "Header")} value={name} onChange={(e) => onName(e.target.value)} spellCheck={false} autoComplete="off" />
      <Input
        secret
        label={t("orgHome.mcp.headerValue", "Value")}
        placeholder={t("orgHome.mcp.headerValuePlaceholder", "Bearer …")}
        value={value}
        onChange={(e) => onValue(e.target.value)}
        autoComplete="off"
        autoFocus
      />
    </div>
  );
}
