import { formatTokens } from "../../format/index.js";
import { AXIS, labelWidth } from "./core.js";

export const finite = (values: readonly number[]): number[] => values.filter(Number.isFinite);

export const hasPoints = (values: readonly number[]): boolean => values.some(Number.isFinite);

export const onlyZero = (values: readonly number[]): boolean => hasPoints(values) && finite(values).every((v) => v === 0);

export function valueExtent(values: readonly number[], also: readonly number[] = []): [number, number] {
  const all = finite([...values, ...also]);
  const lo = Math.min(0, ...all);
  const hi = Math.max(0, ...all);
  if (lo === 0 && hi === 0) return [0, 1];
  return [lo * 1.08, hi * 1.08];
}

export const compactAxis = (v: number): string => formatTokens(v);

export function axisRoom(
  labels: readonly string[],
  { gap = 12, min = 52, max = Infinity }: { gap?: number | undefined; min?: number | undefined; max?: number | undefined } = {},
): number {
  const widest = Math.max(0, ...labels.map(labelWidth));
  return Math.round(Math.min(max, Math.max(min, gap + widest + 4)));
}

export function fitLabel(label: string, room: number, fontSize: number = AXIS.label.fontSize): string {
  const scale = fontSize / AXIS.label.fontSize;
  if (labelWidth(label) * scale <= room) return label;
  const chars = [...label];
  let out = "";
  for (const ch of chars) {
    if (labelWidth(`${out}${ch}…`) * scale > room) break;
    out += ch;
  }
  return `${out.trimEnd()}…`;
}
