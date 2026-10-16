import type { ReactElement, ReactNode } from "react";
import { AlertDialog } from "../../components/dialog/dialog";
import { useUiText } from "../../i18n/context";

export const KAESO_DELETE_RESOURCE_VARIANTS = {
  confirm: {
    typed: { classes: "", description: "The person types the resource's name (default)" },
    simple: { classes: "", description: "A confirm button only, for reversible archives" },
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

export function DeleteResource({ kind, name, consequence, verb, confirm = "typed", onConfirm, trigger, open, onOpenChange, inline }: DeleteResourceProps) {
  const t = useUiText();
  return (
    <AlertDialog
      title={
        verb === undefined ? t("deleteResource.deleteTitle", "Delete this {kind}?", { kind }) : t("deleteResource.title", "{verb} this {kind}?", { verb, kind })
      }
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
}
DeleteResource.displayName = "DeleteResource";
