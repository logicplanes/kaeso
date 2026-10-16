import { useRef } from "react";
import { Avatar, Button } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import { toast } from "~/shell/toast";

const TYPES = ["image/png", "image/jpeg", "image/webp"];

const SIZE = 256;

export async function toPictureUpload(file: File): Promise<{ content_type: "image/webp" | "image/png"; data: string }> {
  const bitmap = await createImageBitmap(file);
  const side = Math.min(bitmap.width, bitmap.height);
  const canvas = document.createElement("canvas");
  canvas.width = SIZE;
  canvas.height = SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error(t("kaeso.picture.unsupported", "This browser can't prepare the picture."));
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(bitmap, (bitmap.width - side) / 2, (bitmap.height - side) / 2, side, side, 0, 0, SIZE, SIZE);
  bitmap.close();
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/webp", 0.9));
  if (!blob) throw new Error(t("kaeso.picture.unsupported", "This browser can't prepare the picture."));
  const content_type = blob.type === "image/webp" ? "image/webp" : "image/png";
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return { content_type, data: btoa(binary) };
}

export function PictureControl({
  name,
  src,
  kind = "person",
  onPick,
  onRemove,
  busy,
  disabledReason,
}: {
  name: string;
  src: string | null | undefined;
  kind?: "person" | "org";
  onPick: (file: File) => void;
  onRemove: () => void;
  busy?: boolean;
  disabledReason?: string | undefined;
}) {
  const input = useRef<HTMLInputElement>(null);
  return (
    <div className="flex items-center gap-12">
      <Avatar name={name} {...(src ? { src } : {})} kind={kind} size={40} decorative />
      <input
        ref={input}
        type="file"
        accept={TYPES.join(",")}
        hidden
        onChange={(e) => {
          const file = e.target.files?.[0];
          e.target.value = "";
          if (!file) return;
          if (TYPES.includes(file.type)) onPick(file);
          else toast.error(t("kaeso.picture.wrongType", "Choose a PNG, JPEG or WebP image."));
        }}
      />
      <Button size="sm" loading={busy} disabled={!!disabledReason} {...(disabledReason ? { disabledReason } : {})} onClick={() => input.current?.click()}>
        {src ? t("kaeso.picture.replace", "Replace") : t("kaeso.picture.upload", "Upload")}
      </Button>
      {src && !disabledReason && (
        <Button size="sm" variant="ghost" onClick={onRemove}>
          {t("common.remove", "Remove")}
        </Button>
      )}
    </div>
  );
}
