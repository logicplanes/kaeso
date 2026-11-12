import { useContext, useMemo } from "react";
import { createFormatters, plainFormatters, type Formatters } from "../format/index.js";
import { LocaleContext, NO_PROVIDER } from "./context.js";

/** The formatters in the nearest `LocaleProvider`'s language and catalog. Use them in components instead of the plain functions, so a server rendering several requests at once formats each in its own language. Without a provider they are the plain formatters. */
export function useFormatters(): Formatters {
  const value = useContext(LocaleContext);
  return useMemo(() => (value === NO_PROVIDER ? plainFormatters() : createFormatters(value)), [value]);
}
