import { useEffect, useState } from "react";
import { formatLocale } from "@logicplanes/kaeso-ui/format";
import type { TimelineBucket, UsageRow } from "~/api/generated/types";
import type { TimelineStep } from "~/api/queries/usage";

export interface Point {
  t: Date;
  row: UsageRow | undefined;
}

export const STEP_MS: Record<TimelineStep, number> = { minute: 60_000, hour: 3_600_000 };

const asRow = (b: TimelineBucket): UsageRow => ({
  key: b.start,
  requests: b.requests,
  errors: b.errors,
  input_tokens: b.input_tokens,
  output_tokens: b.output_tokens,
  cache_read_tokens: b.cache_read_tokens,
  cache_write_tokens: b.cache_write_tokens,
  reasoning_tokens: b.reasoning_tokens,
  cost: b.cost ?? null,
  tool_calls: 0,
  tool_call_errors: 0,
});

/** Every step from `from` up to `to`, with the buckets the API returned and nothing for the steps it left out. */
export function fill(buckets: TimelineBucket[] | undefined, from: number, to: number, step: TimelineStep): Point[] {
  const at = new Map((buckets ?? []).map((b) => [Date.parse(b.start), asRow(b)]));
  const size = STEP_MS[step];
  const out: Point[] = [];
  for (let t = Math.floor(from / size) * size; t < to; t += size) out.push({ t: new Date(t), row: at.get(t) });
  return out;
}

export const iso = (ms: number) => new Date(ms).toISOString();

export function useMinute(): number {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 60_000) * 60_000);
  useEffect(() => {
    const id = setInterval(() => setNow(Math.floor(Date.now() / 60_000) * 60_000), 15_000);
    return () => clearInterval(id);
  }, []);
  return now;
}

const formats = new Map<string, Intl.DateTimeFormat>();

export function utcTimeFormat(options: Intl.DateTimeFormatOptions): (d: Date) => string {
  return (d) => {
    const key = `${formatLocale()}|${JSON.stringify(options)}`;
    let f = formats.get(key);
    if (!f) {
      f = new Intl.DateTimeFormat(formatLocale(), { ...options, timeZone: "UTC" });
      formats.set(key, f);
    }
    return f.format(d);
  };
}
