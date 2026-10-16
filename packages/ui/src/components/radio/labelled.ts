import type { ReactNode } from "react";

export function labelled(
  id: string,
  label: ReactNode,
  description: ReactNode,
  own: { "aria-label"?: string | undefined; "aria-labelledby"?: string | undefined; "aria-describedby"?: string | undefined },
) {
  const labelId = `${id}-name`;
  const descriptionId = `${id}-description`;
  const describedBy = [own["aria-describedby"], description ? descriptionId : undefined].filter(Boolean).join(" ");
  return {
    labelId,
    descriptionId,
    aria: {
      ...(own["aria-label"] || own["aria-labelledby"] || !label ? {} : { "aria-labelledby": labelId }),
      ...(describedBy ? { "aria-describedby": describedBy } : {}),
    },
  };
}
