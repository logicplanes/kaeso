import { Link } from "@logicplanes/kaeso-ui";
import type { Locale } from "~/i18n/locales";
import { useSiteLocale, useText } from "~/i18n/text";

const LEGAL: Record<Locale, { notice: string; privacy: string }> = {
  en: { notice: "https://kaeso.app/impressum", privacy: "https://kaeso.app/datenschutz" },
  de: { notice: "https://kaeso.app/de/impressum", privacy: "https://kaeso.app/de/datenschutz" },
  "zh-CN": { notice: "https://kaeso.app/zh-CN/impressum", privacy: "https://kaeso.app/zh-CN/datenschutz" },
};

export function LegalLinks({ className = "" }: { className?: string }) {
  const t = useText();
  const links = LEGAL[useSiteLocale()];
  return (
    <nav aria-label={t("legal.label")} className={`flex flex-wrap gap-x-16 gap-y-8 ${className}`}>
      <Link href={links.notice} variant="quiet">
        {t("legal.notice")}
      </Link>
      <Link href={links.privacy} variant="quiet">
        {t("legal.privacy")}
      </Link>
    </nav>
  );
}
