import { forwardRef, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { Badge } from "../badge/badge.js";
import { Tooltip } from "../tooltip/tooltip.js";
import type { RightSource, RightsGroup } from "./rights.js";
import { useUiText } from "../../i18n/context.js";

export const KAESO_EFFECTIVE_ACCESS_VARIANTS = {
  layout: {
    rows: {
      classes: "flex flex-col @auth:grid @auth:grid-cols-term @auth:gap-x-16",
      description: "A row per group: its name, what is granted in words, and how many (default)",
    },
  },
} as const;
export const KAESO_EFFECTIVE_ACCESS_DEFAULT_VARIANTS = { layout: "rows" } as const;

export interface EffectiveAccessProps {
  groups: RightsGroup[];
  sources?: RightSource[] | undefined;
  action?: ReactNode | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

export const EffectiveAccess = forwardRef<HTMLDivElement, EffectiveAccessProps>(function EffectiveAccess(
  { groups, sources, action, "aria-label": ariaLabel, className },
  ref,
) {
  const t = useUiText();
  const empty = groups.filter((g) => !g.rights.some((r) => r.granted));
  return (
    <div
      ref={ref}
      role="group"
      aria-label={ariaLabel ?? t("effectiveAccess.access", "Access")}
      className={cn("@container flex min-w-0 flex-col gap-12", className)}
    >
      {!!sources?.length && (
        <span className="flex flex-wrap items-center gap-4">
          <span className="mr-4 text-small text-fg-secondary">{t("effectiveAccess.from", "From")}</span>
          {sources.map((s) =>
            s.detail ? (
              <Tooltip key={s.label} content={s.detail}>
                <Badge tabIndex={0} className="focus-ring">
                  {s.label}
                </Badge>
              </Tooltip>
            ) : (
              <Badge key={s.label}>{s.label}</Badge>
            ),
          )}
        </span>
      )}
      <dl className={KAESO_EFFECTIVE_ACCESS_VARIANTS.layout.rows.classes}>
        {groups
          .filter((g) => g.rights.some((r) => r.granted))
          .map((g) => {
            const granted = g.rights.filter((r) => r.granted);
            const words =
              granted.length === g.rights.length
                ? t("effectiveAccess.everyRight", "Every right")
                : granted.map((r) => r.label ?? r.code).join(t("effectiveAccess.listSeparator", ", "));
            return (
              <Row
                key={g.label}
                term={g.label}
                count={t("effectiveAccess.grantedOfTotal", "{granted} of {total}", { granted: granted.length, total: g.rights.length })}
              >
                {words}
              </Row>
            );
          })}
        {empty.length > 0 && (
          <Row term={empty.length === groups.length ? t("effectiveAccess.noRights", "No rights") : t("effectiveAccess.noneIn", "None in")} quiet>
            {empty.map((g) => g.label).join(t("effectiveAccess.listSeparator", ", "))}
          </Row>
        )}
      </dl>
      {action && <div className="flex">{action}</div>}
    </div>
  );
});
EffectiveAccess.displayName = "EffectiveAccess";

function Row({ term, count, quiet, children }: { term: string; count?: string | undefined; quiet?: boolean | undefined; children: ReactNode }) {
  return (
    <div className="flex min-h-40 flex-col gap-2 py-8 not-first:hairline-t @auth:col-span-2 @auth:grid @auth:grid-cols-subgrid @auth:items-baseline @auth:gap-16">
      <dt className={cn("shrink-0 text-small font-medium @auth:min-w-140", quiet ? "text-fg-secondary" : "text-fg")}>{term}</dt>
      <dd className="flex min-w-0 flex-1 items-baseline gap-12">
        <span className={cn("min-w-0 flex-1 text-small text-pretty", quiet ? "text-fg-tertiary" : "text-fg-secondary")}>{children}</span>
        {count && <span className="shrink-0 text-small text-fg-tertiary tabular">{count}</span>}
      </dd>
    </div>
  );
}
