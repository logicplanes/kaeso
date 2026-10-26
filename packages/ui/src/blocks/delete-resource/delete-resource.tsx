import { forwardRef, type ReactElement, type ReactNode } from "react";
import { AlertDialog } from "../../components/dialog/dialog.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_DELETE_RESOURCE_VARIANTS = {
  confirm: {
    typed: { classes: "", description: "The person types the resource's name, for one that holds a lot: a project, an organization (default)" },
    simple: { classes: "", description: "A confirm button only, for a resource that holds little: a key, an alias" },
  },
} as const;
export const KAESO_DELETE_RESOURCE_DEFAULT_VARIANTS = { confirm: "typed" } as const;

export interface DeleteResourceProps {
  kind: string;
  name: string;
  consequence: ReactNode;
  verb?: string | undefined;
  confirm?: keyof typeof KAESO_DELETE_RESOURCE_VARIANTS.confirm | undefined;
  onConfirm: () => void | Promise<void>;
  trigger?: ReactElement | undefined;
  open?: boolean | undefined;
  onOpenChange?: ((open: boolean) => void) | undefined;
  inline?: boolean | undefined;
}

export const DeleteResource = forwardRef<HTMLDivElement, DeleteResourceProps>(function DeleteResource(
  { kind, name, consequence, verb, confirm = "typed", onConfirm, trigger, open, onOpenChange, inline },
  ref,
) {
  const t = useUiText();
  return (
    <AlertDialog
      ref={ref}
      title={verb === undefined ? t("deleteResource.deleteTitle", "Delete “{name}”?", { name }) : t("deleteResource.title", "{verb} “{name}”?", { verb, name })}
      description={consequence}
      confirmLabel={
        verb === undefined ? t("deleteResource.deleteConfirm", "Delete {kind}", { kind }) : t("deleteResource.confirm", "{verb} {kind}", { verb, kind })
      }
      onConfirm={onConfirm}
      tone="danger"
      {...(confirm === "typed" ? { confirmText: name } : {})}
      {...(trigger ? { trigger } : {})}
      {...(open !== undefined ? { open } : {})}
      {...(onOpenChange ? { onOpenChange } : {})}
      {...(inline ? { inline } : {})}
    />
  );
});
DeleteResource.displayName = "DeleteResource";
