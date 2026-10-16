import { t } from "~/i18n";

export const eitherOf = (first: string, second: string): string => t("auth.eitherOf", "{first} or {second}", { first, second });
