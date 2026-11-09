import { forwardRef } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { useUiText } from "../../i18n/context.js";
import { Button } from "../button/button.js";
import { Tag } from "../tag/tag.js";
import { Thumbnail } from "../thumbnail/thumbnail.js";

/** A file sent with a prompt: an image shows as a thumbnail, anything else by its name. */
export interface Attachment {
  id: string;
  name: string;
  /** The file's media type, such as `image/png` or `application/pdf`. */
  type: string;
  /** Where the image can be drawn from: a `blob:` or `data:` URL. */
  src?: string | undefined;
}

export const KAESO_ATTACHMENTS_VARIANTS = {
  align: {
    start: { classes: "justify-start", description: "At the start of the line: in the composer, above the prompt (default)" },
    end: { classes: "justify-end", description: "At the end of the line: above a sent prompt's bubble" },
  },
} as const;
export const KAESO_ATTACHMENTS_DEFAULT_VARIANTS = { align: "start" } as const;

const isImage = (a: Attachment) => a.type.startsWith("image/") && !!a.src;

export interface AttachmentsProps {
  items: readonly Attachment[];
  onRemove?: ((id: string) => void) | undefined;
  align?: keyof typeof KAESO_ATTACHMENTS_VARIANTS.align | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

export const Attachments = forwardRef<HTMLUListElement, AttachmentsProps>(function Attachments(
  { items, onRemove, align = "start", "aria-label": ariaLabel, className },
  ref,
) {
  const t = useUiText();
  if (items.length === 0) return null;
  return (
    <ul
      ref={ref}
      aria-label={ariaLabel ?? t("attachments.label", "Attachments")}
      className={cn("flex min-w-0 flex-wrap items-center gap-8", KAESO_ATTACHMENTS_VARIANTS.align[align].classes, className)}
    >
      {items.map((a) => {
        const remove = t("attachments.remove", "Remove {name}", { name: a.name });
        return (
          <li key={a.id} className="relative flex min-w-0">
            {isImage(a) ? (
              <>
                <Thumbnail src={a.src ?? ""} alt={a.name} size="sm" />
                {onRemove && (
                  <Button
                    size="xs"
                    variant="secondary"
                    icon={icons.close}
                    aria-label={remove}
                    onClick={() => onRemove(a.id)}
                    className="absolute top-4 right-4"
                  />
                )}
              </>
            ) : (
              <Tag
                icon={a.type === "application/pdf" ? icons.pdf : a.type.startsWith("audio/") ? icons.audio : icons.file}
                {...(onRemove ? { onRemove: () => onRemove(a.id), removeLabel: remove } : {})}
                className="max-w-240"
              >
                {a.name}
              </Tag>
            )}
          </li>
        );
      })}
    </ul>
  );
});
Attachments.displayName = "Attachments";
