import { useMemo, useState, type ReactNode } from "react";
import { Input, Select, Switch } from "@logicplanes/kaeso-ui";

type Control =
  | { type: "select"; options: string[]; default: string }
  | { type: "text"; default: string }
  | { type: "boolean"; default: boolean }
  | { type: "number"; default: number; min?: number; max?: number; step?: number };

export function Playground({
  controls,
  render,
  code,
}: {
  component: string;
  controls: Record<string, Control>;
  render: (props: Record<string, unknown>) => ReactNode;
  code: (props: Record<string, unknown>) => string;
}) {
  const initial = useMemo(() => Object.fromEntries(Object.entries(controls).map(([k, c]) => [k, c.default])), [controls]);
  const [values, setValues] = useState<Record<string, unknown>>(initial);
  const set = (k: string, v: unknown) => setValues((s) => ({ ...s, [k]: v }));
  return (
    <div className="overflow-hidden rounded-well shadow-frame">
      <div className="grid md:grid-cols-aside">
        <div className="flex min-h-240 items-center justify-center bg-panel p-32">{render(values)}</div>
        <div className="flex flex-col gap-4 bg-raised p-16 md:hairline-l">
          {Object.entries(controls).map(([k, c]) => (
            <div key={k} className="flex min-h-40 items-center justify-between gap-12">
              <span className="mono text-small text-fg-secondary">{k}</span>
              {c.type === "select" ? (
                <Select
                  size="sm"
                  aria-label={k}
                  options={c.options.map((o) => ({ value: o, label: o }))}
                  value={String(values[k])}
                  onValueChange={(v) => set(k, v)}
                />
              ) : c.type === "boolean" ? (
                <Switch size="sm" aria-label={k} checked={!!values[k]} onCheckedChange={(v) => set(k, v)} />
              ) : (
                <Input
                  size="sm"
                  aria-label={k}
                  className="w-160"
                  value={String(values[k])}
                  onChange={(e) => set(k, c.type === "number" ? Number(e.target.value) : e.target.value)}
                />
              )}
            </div>
          ))}
        </div>
      </div>
      <pre className="mono overflow-x-auto bg-fill-1 px-16 py-12 text-small text-fg scrollbar-quiet">{code(values)}</pre>
    </div>
  );
}
