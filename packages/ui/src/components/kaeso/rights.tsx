import { forwardRef, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
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
import { useInset, useRaised } from "../../utils/inset.js";
import { TruncatedText } from "../text/truncated.js";

const STOPS = '[role="checkbox"], [data-right-extra]';

export const KAESO_RIGHTS_MATRIX_VARIANTS = {
  origin: {
    preset: { classes: "text-fg-secondary", description: "Comes from the role" },
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

  const [stop, setStop] = useState(0);
  const roving = useRef(() => {});
  roving.current = () => {
    const el = root.current;
    if (!el || readOnly) return;
    const boxes = [...el.querySelectorAll<HTMLElement>('[role="checkbox"]')];
    const own = boxes[Math.min(stop, boxes.length - 1)];
    for (const w of el.querySelectorAll<HTMLElement>(STOPS)) {
      const tabindex = w === own ? "0" : "-1";
      if (w.getAttribute("tabindex") !== tabindex) w.setAttribute("tabindex", tabindex);
    }
  };
  useLayoutEffect(() => roving.current());
  useLayoutEffect(() => {
    const el = root.current;
    if (!el || readOnly) return;
    const observer = new MutationObserver(() => roving.current());
    observer.observe(el, { subtree: true, childList: true, attributes: true, attributeFilter: ["tabindex"] });
    return () => observer.disconnect();
  }, [readOnly]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (!root.current || readOnly || e.altKey || e.metaKey || e.ctrlKey) return;
    const boxes = [...root.current.querySelectorAll<HTMLElement>('[role="checkbox"]')];
    const row = target.closest<HTMLElement>("[data-right]");
    const box = target.getAttribute("role") === "checkbox" ? target : row?.querySelector<HTMLElement>('[role="checkbox"]');
    if (!box) return;
    if (target !== box) {
      if (e.key === "ArrowLeft" || e.key === "Escape") {
        e.preventDefault();
        box.focus();
      }
      return;
    }
    const i = boxes.indexOf(box);
    const extra = row?.querySelector<HTMLElement>("[data-right-extra]");
    const next =
      e.key === "ArrowDown"
        ? boxes[i + 1]
        : e.key === "ArrowUp"
          ? boxes[i - 1]
          : e.key === "Home"
            ? boxes[0]
            : e.key === "End"
              ? boxes[boxes.length - 1]
              : e.key === "ArrowRight"
                ? extra
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
      onFocus={(e) => {
        const boxes = [...(root.current?.querySelectorAll<HTMLElement>('[role="checkbox"]') ?? [])];
        const i = boxes.indexOf(e.target as HTMLElement);
        if (i >= 0 && i !== stop) setStop(i);
      }}
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
        const changeableOn = changeable.length > 0 && changeable.every((r) => r.granted);
        const allOn = grantedCount === g.rights.length;
        const Header = readOnly ? "div" : "label";
        return (
          <section key={g.label} aria-labelledby={headingId} className="flex min-w-0 flex-col">
            <Header data-right="" className={cn("flex items-start gap-12 px-12 pb-4", !readOnly && changeable.length > 0 && "cursor-pointer")}>
              {!readOnly && (
                <span className="flex h-20 items-center">
                  <CheckboxBox
                    aria-label={
                      q
                        ? t("rightsMatrix.everyRightShownIn", "Every right shown in {group}", { group: g.label })
                        : t("rightsMatrix.everyRightIn", "Every right in {group}", { group: g.label })
                    }
                    checked={allOn}
                    indeterminate={grantedCount > 0 && !allOn}
                    disabled={changeable.length === 0}
                    onCheckedChange={() => apply(groupCascade(all, g.rights, !changeableOn))}
                  />
                </span>
              )}
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="flex items-baseline justify-between gap-12">
                  <span id={headingId} className="min-w-0 wrap-break-word text-body font-semibold text-fg">
                    {g.label}
                  </span>
                  <span className="shrink-0 text-small text-fg-secondary tabular">
                    {t("rightsMatrix.grantedOfTotal", "{granted} of {total}", { granted: grantedCount, total: g.rights.length })}
                  </span>
                </span>
                {g.description && <span className="text-small text-fg-secondary text-pretty">{g.description}</span>}
              </span>
            </Header>
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
  const inset = useInset();
  const raised = useRaised();
  const onRaised = inset || raised;
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
            (readOnly && !r.granted) || r.disabled ? "text-fg-secondary" : "text-fg",
          )}
        >
          <Highlight text={name} query={query} />
        </span>
        {r.label && (
          <span className="hidden h-20 min-w-0 items-center @auth:flex">
            <TruncatedText className="min-w-0 mono text-caption text-fg-tertiary">{r.code}</TruncatedText>
          </span>
        )}
        {r.sensitive && (
          <span className={cn("flex shrink-0 items-center", r.label ? "h-20" : "h-18")}>
            <Tooltip content={t("rightsMatrix.sensitiveHint", "Sensitive: exposes data or is hard to undo")}>
              <span tabIndex={0} data-right-extra="" className="inline-flex shrink-0 rounded-pill text-warning-text focus-ring">
                <Icon icon={icons.warning} size="sm" label={t("rightsMatrix.sensitive", "Sensitive")} />
              </span>
            </Tooltip>
          </span>
        )}
        {hasOrigin && !readOnly && (
          <span
            className={cn(
              "ml-auto flex max-w-160 min-w-80 shrink-0 items-center justify-end text-right text-small",
              r.label ? "min-h-20" : "min-h-18",
              KAESO_RIGHTS_MATRIX_VARIANTS.origin[r.origin ?? "none"].classes,
            )}
          >
            <TruncatedText>{originText[r.origin ?? "none"]}</TruncatedText>
          </span>
        )}
      </span>
      <span id={descId} className="text-small text-pretty text-fg-secondary">
        {r.description}
      </span>
      {needs.length > 0 && (
        <span className="text-caption text-fg-secondary">
          {t("rightsMatrix.needs", "Needs {rights}", { rights: needs.join(t("rightsMatrix.listSeparator", ", ")) })}
        </span>
      )}
      {readOnly && r.granted && !!r.sources?.length && (
        <span className="flex flex-wrap gap-4 pt-4">
          {r.sources.map((s) =>
            s.detail ? (
              <Tooltip key={s.label} content={s.detail}>
                <Badge tabIndex={0} className="cursor-help underline decoration-dotted underline-offset-2 focus-ring">
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
      <div className="flex min-h-56 items-start gap-12 px-12 py-8">
        <span className="flex h-20 w-18 shrink-0 items-center justify-center">
          <Icon
            icon={r.granted ? icons.check : icons.minus}
            size="sm"
            className={r.granted ? "text-fg" : "text-fg-tertiary"}
            label={r.granted ? t("rightsMatrix.granted", "Granted") : t("rightsMatrix.notGranted", "Not granted")}
          />
        </span>
        {text}
      </div>
    );
  }

  const row = (
    <label
      data-right=""
      className={cn(
        "flex min-h-56 items-start gap-12 rounded-item px-12 py-8 transition-control",
        r.disabled ? "cursor-not-allowed" : cn("cursor-pointer", onRaised ? "hover:bg-raised-hover" : "hover:bg-fill-1"),
        flashed && (onRaised ? "bg-raised-pressed hover:bg-raised-pressed" : "bg-fill-2 hover:bg-fill-2"),
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
