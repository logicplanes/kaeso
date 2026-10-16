import { forwardRef, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { CheckboxBox } from "../checkbox/checkbox.js";
import { Tooltip } from "../tooltip/tooltip.js";
import { SearchInput } from "../input/input.js";
import { Empty } from "../empty/empty.js";
import { Badge } from "../badge/badge.js";
import { Highlight } from "../combobox/combobox.js";
import { groupCascade, rightsCascade, type RightChange } from "./rights-logic.js";
import { useUiText, type UiText } from "../../i18n/context.js";

export const KAESO_RIGHTS_MATRIX_VARIANTS = {
  origin: {
    preset: { classes: "text-fg-tertiary", description: "Comes from the role" },
    added: { classes: "text-success-text", description: "Added on top of the role" },
    removed: { classes: "text-danger-text", description: "Removed from the role" },
    none: { classes: "", description: "Not granted" },
  },
  mode: {
    edit: { classes: "", description: "Checkboxes, group toggles and requirement hints (default)" },
    readOnly: { classes: "", description: "A mark per right and, under it, the roles that grant it" },
  },
} as const;
export const KAESO_RIGHTS_MATRIX_DEFAULT_VARIANTS = { origin: "none", mode: "edit" } as const;

type Origin = keyof typeof KAESO_RIGHTS_MATRIX_VARIANTS.origin;

export interface RightSource {
  label: string;
  detail?: string | undefined;
}

export interface Right {
  code: string;
  label?: string | undefined;
  description: string;
  granted: boolean;
  origin?: Origin | undefined;
  sensitive?: boolean | undefined;
  requires?: string[] | undefined;
  sources?: RightSource[] | undefined;
  disabled?: boolean | undefined;
  disabledReason?: string | undefined;
}

export interface RightsGroup {
  label: string;
  description?: string | undefined;
  rights: Right[];
}

export interface RightsMatrixProps {
  groups: RightsGroup[];
  onChange?: ((code: string, granted: boolean) => void) | undefined;
  onRightsChange?: ((changes: RightChange[]) => void) | undefined;
  readOnly?: boolean | undefined;
  searchable?: boolean | undefined;
  originLabels?: Partial<Record<Origin, string>> | undefined;
  "aria-label"?: string | undefined;
  className?: string | undefined;
}

function originTexts(t: UiText): Record<Origin, string> {
  return { preset: t("rightsMatrix.fromRole", "From role"), added: t("rightsMatrix.added", "Added"), removed: t("rightsMatrix.removed", "Removed"), none: "" };
}

const matches = (q: string, ...texts: (string | undefined)[]) => texts.some((t) => t?.toLowerCase().includes(q));

export const RightsMatrix = forwardRef<HTMLDivElement, RightsMatrixProps>(function RightsMatrix(
  { groups, onChange, onRightsChange, readOnly, searchable, originLabels, "aria-label": ariaLabel, className },
  ref,
) {
  const t = useUiText();
  const [query, setQuery] = useState("");
  const [announcement, setAnnouncement] = useState("");
  const [flashed, setFlashed] = useState<ReadonlySet<string>>(() => new Set());
  const flashTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => () => clearTimeout(flashTimer.current), []);
  const root = useRef<HTMLDivElement | null>(null);
  const baseId = useId();
  const all = useMemo(() => groups.flatMap((g) => g.rights), [groups]);
  const names = useMemo(() => new Map(all.map((r) => [r.code, r.label ?? r.code])), [all]);
  const universal = useMemo(() => {
    if (all.length < 3) return new Set<string>();
    const required = new Set(all.flatMap((r) => r.requires ?? []));
    return new Set([...required].filter((c) => all.every((r) => r.code === c || r.requires?.includes(c))));
  }, [all]);
  const hasOrigins = all.some((r) => r.origin !== undefined);
  const originText = { ...originTexts(t), ...originLabels };
  const q = query.trim().toLowerCase();
  const shown = groups
    .map((g) => (!q || matches(q, g.label) ? g : { ...g, rights: g.rights.filter((r) => matches(q, r.label, r.code, r.description)) }))
    .filter((g) => g.rights.length > 0);

  const apply = (changes: RightChange[]) => {
    if (!changes.length) return;
    if (onRightsChange) onRightsChange(changes);
    else if (onChange) for (const c of changes) onChange(c.code, c.granted);
    const [first, ...rest] = changes;
    const also = rest.map((c) => names.get(c.code) ?? c.code);
    setAnnouncement(
      also.length && first
        ? first.granted
          ? t("rightsMatrix.alsoGranted", "Also granted {rights}", { rights: also.join(t("rightsMatrix.listSeparator", ", ")) })
          : t("rightsMatrix.alsoRemoved", "Also removed {rights}", { rights: also.join(t("rightsMatrix.listSeparator", ", ")) })
        : "",
    );
    clearTimeout(flashTimer.current);
    setFlashed(new Set(rest.map((c) => c.code)));
    if (rest.length) flashTimer.current = setTimeout(() => setFlashed(new Set()), 1200);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.getAttribute("role") !== "checkbox" || !root.current) return;
    const boxes = [...root.current.querySelectorAll<HTMLElement>('[role="checkbox"]')];
    const i = boxes.indexOf(target);
    const next =
      e.key === "ArrowDown"
        ? boxes[i + 1]
        : e.key === "ArrowUp"
          ? boxes[i - 1]
          : e.key === "Home"
            ? boxes[0]
            : e.key === "End"
              ? boxes[boxes.length - 1]
              : undefined;
    if (!next) return;
    e.preventDefault();
    next.focus();
  };

  return (
    <div
      ref={(el) => {
        root.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      role="group"
      aria-label={ariaLabel ?? t("rightsMatrix.rights", "Rights")}
      onKeyDown={onKeyDown}
      className={cn("@container flex min-w-0 flex-col gap-24", className)}
    >
      {searchable && (
        <SearchInput
          className="w-full"
          aria-label={t("rightsMatrix.searchRights", "Search rights")}
          placeholder={t("rightsMatrix.searchRights", "Search rights")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onClear={() => setQuery("")}
        />
      )}
      {shown.length === 0 && (
        <Empty
          size="compact"
          title={t("rightsMatrix.noMatch", "No rights match")}
          description={t("rightsMatrix.noMatchHint", "Try a right's name, its code, or a word from its description.")}
        />
      )}
      {shown.map((g, gi) => {
        const headingId = `${baseId}-g${gi}`;
        const grantedCount = g.rights.filter((r) => r.granted).length;
        const changeable = g.rights.filter((r) => !r.disabled);
        const allOn = changeable.length > 0 && changeable.every((r) => r.granted);
        const someOn = changeable.some((r) => r.granted);
        return (
          <section key={g.label} aria-labelledby={headingId} className="flex min-w-0 flex-col">
            <div className="flex items-start gap-12 pb-4">
              {!readOnly && (
                <span className="flex h-20 items-center">
                  <CheckboxBox
                    aria-label={
                      q
                        ? t("rightsMatrix.everyRightShownIn", "Every right shown in {group}", { group: g.label })
                        : t("rightsMatrix.everyRightIn", "Every right in {group}", { group: g.label })
                    }
                    checked={allOn}
                    indeterminate={someOn && !allOn}
                    disabled={changeable.length === 0}
                    onCheckedChange={() => apply(groupCascade(all, g.rights, !allOn))}
                  />
                </span>
              )}
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="flex items-baseline justify-between gap-12">
                  <span id={headingId} className="min-w-0 wrap-break-word text-body font-semibold text-fg">
                    {g.label}
                  </span>
                  <span className="shrink-0 text-small text-fg-tertiary tabular">
                    {t("rightsMatrix.grantedOfTotal", "{granted} of {total}", { granted: grantedCount, total: g.rights.length })}
                  </span>
                </span>
                {g.description && <span className="text-small text-fg-secondary text-pretty">{g.description}</span>}
              </span>
            </div>
            {g.rights.map((r) => (
              <RightRow
                key={r.code}
                right={r}
                readOnly={!!readOnly}
                query={q}
                hasOrigin={hasOrigins}
                originText={originText}
                names={names}
                universal={universal}
                flashed={flashed.has(r.code)}
                onToggle={(granted) => apply(rightsCascade(all, r.code, granted))}
              />
            ))}
          </section>
        );
      })}
      <span aria-live="polite" className="sr-only">
        {announcement}
      </span>
    </div>
  );
});
RightsMatrix.displayName = "RightsMatrix";

function RightRow({
  right: r,
  readOnly,
  query,
  hasOrigin,
  originText,
  names,
  universal,
  flashed,
  onToggle,
}: {
  right: Right;
  readOnly: boolean;
  query: string;
  hasOrigin: boolean;
  originText: Record<Origin, string>;
  names: Map<string, string>;
  universal: ReadonlySet<string>;
  flashed: boolean;
  onToggle: (granted: boolean) => void;
}) {
  const t = useUiText();
  const descId = useId();
  const nameId = useId();
  const name = r.label ?? r.code;
  const needs = readOnly ? [] : (r.requires ?? []).filter((c) => !universal.has(c)).map((c) => names.get(c) ?? c);
  const text: ReactNode = (
    <span className="flex min-w-0 flex-1 flex-col gap-2">
      <span className="flex min-w-0 items-start gap-8">
        <span
          id={nameId}
          className={cn(
            "min-w-0 wrap-break-word",
            r.label ? "text-body font-medium" : "mono text-small",
            readOnly && !r.granted ? "text-fg-tertiary" : "text-fg",
          )}
        >
          <Highlight text={name} query={query} />
        </span>
        {r.label && (
          <span className="hidden h-20 min-w-0 items-center @auth:flex">
            <span className="min-w-0 truncate mono text-caption text-fg-tertiary">{r.code}</span>
          </span>
        )}
        {r.sensitive && (
          <span className={cn("flex shrink-0 items-center", r.label ? "h-20" : "h-18")}>
            <Tooltip content={t("rightsMatrix.sensitiveHint", "Sensitive: exposes data or is hard to undo")}>
              <span tabIndex={0} className="inline-flex shrink-0 rounded-pill text-warning-text focus-ring">
                <Icon icon={icons.warning} size="sm" label={t("rightsMatrix.sensitive", "Sensitive")} />
              </span>
            </Tooltip>
          </span>
        )}
        {hasOrigin && !readOnly && (
          <span
            className={cn(
              "ml-auto flex w-80 shrink-0 items-center justify-end text-right text-small",
              r.label ? "h-20" : "h-18",
              KAESO_RIGHTS_MATRIX_VARIANTS.origin[r.origin ?? "none"].classes,
            )}
          >
            {originText[r.origin ?? "none"]}
          </span>
        )}
      </span>
      <span id={descId} className={cn("text-small text-pretty", readOnly && !r.granted ? "text-fg-tertiary" : "text-fg-secondary")}>
        {r.description}
      </span>
      {needs.length > 0 && (
        <span className="text-caption text-fg-tertiary">
          {t("rightsMatrix.needs", "Needs {rights}", { rights: needs.join(t("rightsMatrix.listSeparator", ", ")) })}
        </span>
      )}
      {readOnly && r.granted && !!r.sources?.length && (
        <span className="flex flex-wrap gap-4 pt-4">
          {r.sources.map((s) =>
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
    </span>
  );

  if (readOnly) {
    return (
      <div className="-mx-8 flex min-h-52 items-start gap-12 px-8 py-8">
        <span className="flex h-20 w-18 shrink-0 items-center justify-center">
          <Icon
            icon={r.granted ? icons.check : icons.minus}
            size="sm"
            className={r.granted ? "text-fg" : "text-fg-disabled"}
            label={r.granted ? t("rightsMatrix.granted", "Granted") : t("rightsMatrix.notGranted", "Not granted")}
          />
        </span>
        {text}
      </div>
    );
  }

  const row = (
    <label
      className={cn(
        "-mx-8 flex min-h-52 cursor-pointer items-start gap-12 rounded-item px-8 py-8 transition-control hover:bg-fill-1",
        flashed && "bg-fill-2 hover:bg-fill-2",
        r.disabled && "cursor-not-allowed",
      )}
    >
      <span className="flex h-20 items-center">
        <CheckboxBox checked={r.granted} disabled={r.disabled} onCheckedChange={onToggle} aria-labelledby={nameId} aria-describedby={descId} />
      </span>
      {text}
    </label>
  );
  return r.disabled && r.disabledReason ? <Tooltip content={r.disabledReason}>{row}</Tooltip> : row;
}
