import { locale } from "~/i18n";

const collators = new Map<string, Intl.Collator>();

export function compareNames(a: string, b: string): number {
  const tag = locale();
  let collator = collators.get(tag);
  if (!collator) collators.set(tag, (collator = new Intl.Collator(tag, { sensitivity: "base", numeric: true })));
  return collator.compare(a, b);
}
