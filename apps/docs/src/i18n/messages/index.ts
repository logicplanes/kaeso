import type { Locale } from "../locales.ts";
import { de } from "./de.ts";
import { en, type MessageKey, type Messages } from "./en.ts";
import { zhCN } from "./zh-CN.ts";

export type { MessageKey, Messages };

export const MESSAGES: Readonly<Record<Locale, Messages>> = { en, de, "zh-CN": zhCN };
