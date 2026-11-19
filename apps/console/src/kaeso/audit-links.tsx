import { Button, Section } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

/**
 * A person's sheet's way into the audit log at `log`: their own changes (the `actor` filter) and the changes to them
 * (the `target` filter). The caller shows it only to people who may read that log.
 */
export function AuditLinks({ log, user }: { log: string; user: string }) {
  return (
    <Section
      title={t("kaeso.auditLinks.title", "Audit log")}
      description={t("kaeso.auditLinks.hint", "What they changed, and what was changed about them.")}
      spacing="none"
    >
      <div className="flex flex-wrap gap-8">
        <Button size="sm" href={`${log}?actor=${user}`}>
          {t("kaeso.auditLinks.theirs", "Their changes")}
        </Button>
        <Button size="sm" href={`${log}?target=${user}`}>
          {t("kaeso.auditLinks.toThem", "Changes to them")}
        </Button>
      </div>
    </Section>
  );
}
