import { useRef } from "react";
import { Button, Input, Select, icons } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

const MAX_KEY_BYTES = 16 * 1024;

export function serviceAccountBlocked(secret: string): string | undefined {
  const key = secret.trim();
  if (!key) return t("kaeso.providerKey.serviceAccountBlocked", "Paste or choose the JSON key");
  if (!key.startsWith("{") || !key.endsWith("}")) return t("kaeso.providerKey.serviceAccountNotJson", "Paste the whole JSON key file");
  return undefined;
}

export function ServiceAccountFields({
  secret,
  onSecret,
  locations,
  location,
  onLocation,
  name,
  onName,
}: {
  secret: string;
  onSecret: (secret: string) => void;
  locations: readonly string[];
  location: string;
  onLocation: (location: string) => void;
  name: string | null;
  onName: (name: string) => void;
}) {
  const file = useRef<HTMLInputElement>(null);
  return (
    <div className="flex flex-col gap-12">
      <div className="flex items-end gap-8">
        <Input
          className="min-w-0 flex-1"
          secret
          label={t("kaeso.providerKey.serviceAccountKey", "JSON key")}
          placeholder={t("kaeso.providerKey.serviceAccountPlaceholder", "Paste the key file's contents")}
          value={secret}
          onChange={(e) => onSecret(e.target.value)}
          autoComplete="off"
          spellCheck={false}
          autoFocus
        />
        <input
          ref={file}
          type="file"
          accept="application/json,.json"
          hidden
          onChange={(e) => {
            const chosen = e.target.files?.[0];
            e.target.value = "";
            if (!chosen) return;
            if (chosen.size > MAX_KEY_BYTES) {
              toast.error(t("kaeso.providerKey.serviceAccountTooLarge", "That file is too large to be a JSON key."));
              return;
            }
            void chosen.text().then(onSecret);
          }}
        />
        <Button leadingIcon={icons.upload} onClick={() => file.current?.click()}>
          {t("kaeso.providerKey.serviceAccountChoose", "Choose file")}
        </Button>
      </div>
      <Select
        fullWidth
        label={t("kaeso.providerKey.location", "Location")}
        description={t("kaeso.providerKey.locationHint", "Where Google runs the requests; global serves the most models.")}
        value={location}
        onValueChange={(v) => onLocation(v as string)}
        options={locations.map((l) => ({ value: l, label: l }))}
      />
      {name !== null && (
        <Input
          label={t("common.name", "Name")}
          placeholder={t("kaeso.providerKey.namePlaceholder", "Production")}
          value={name}
          onChange={(e) => onName(e.target.value)}
          maxLength={100}
        />
      )}
    </div>
  );
}
