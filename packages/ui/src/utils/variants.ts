export type VariantAxis = Record<string, { readonly classes: string; readonly description: string }>;

export function resolveVariant<A extends VariantAxis>(axis: A, value: keyof A | undefined, fallback: keyof A): string {
  const entry = (value !== undefined ? axis[value] : undefined) ?? axis[fallback];
  return entry?.classes ?? "";
}
