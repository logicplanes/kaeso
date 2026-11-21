import type { ReactNode } from "react";

// Inter's tabular figures give a hyphen a figure's width, so a name such as "gpt-5-mini" or a version such as
// "1.4.0-rc.2" reads spaced out. Dates, times and numbers keep them, so their figures line up.
const HYPHENATED_NAME = /\p{L}-|-\p{L}/u;

export const tabularText = (text: string) => !HYPHENATED_NAME.test(text);

function plainText(node: ReactNode): string | undefined {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (node === null || node === undefined || typeof node === "boolean") return "";
  if (Array.isArray(node)) {
    let text = "";
    for (const part of node as ReactNode[]) {
      const s = plainText(part);
      if (s === undefined) return undefined;
      text += s;
    }
    return text;
  }
  return undefined;
}

/** `tabular`, unless the text is a hyphenated name or version. Elements keep it: what they show can't be read here. */
export function figures(node: ReactNode): "tabular" | undefined {
  const text = plainText(node);
  return text === undefined || tabularText(text) ? "tabular" : undefined;
}
