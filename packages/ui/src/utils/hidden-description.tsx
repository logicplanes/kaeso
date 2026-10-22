import { useId, type ReactNode } from "react";

interface HiddenDescription {
  id: string | undefined;
  node: ReactNode;
}

export function useHiddenDescription(text: string | undefined): HiddenDescription {
  const id = useId();
  if (!text) return { id: undefined, node: null };
  return {
    id,
    node: (
      <span id={id} hidden>
        {text}
      </span>
    ),
  };
}

export const joinIds = (...ids: (string | false | null | undefined)[]): string | undefined => ids.filter(Boolean).join(" ") || undefined;

export const showsText = (node: ReactNode): boolean =>
  node !== undefined && node !== null && node !== false && node !== true && !(typeof node === "string" && node.trim() === "");
