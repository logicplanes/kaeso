import type { PlaygroundModel } from "~/api/queries/playground";
import type { ResponseUsage } from "./responses";

const SCALE = 12;
const UNIT = 10n ** BigInt(SCALE);

function units(value: string | null | undefined): bigint | null {
  if (value === null || value === undefined) return null;
  const m = /^\s*(\d*)(?:\.(\d*))?\s*$/.exec(value);
  if (!m || (!m[1] && !m[2])) return null;
  const fraction = (m[2] ?? "").slice(0, SCALE).padEnd(SCALE, "0");
  return BigInt(m[1] || "0") * UNIT + BigInt(fraction);
}

function decimal(value: bigint): string {
  const micro = (value + 500_000n) / 1_000_000n;
  const whole = micro / 1_000_000n;
  return `${whole}.${(micro % 1_000_000n).toString().padStart(6, "0")}`;
}

export function replyCost(
  usage: ResponseUsage,
  prices: Pick<PlaygroundModel, "input_price" | "output_price" | "cache_read_price" | "cache_write_price"> | null,
): string | null {
  if (usage.cost !== undefined && units(usage.cost) !== null) return usage.cost;
  if (!prices) return null;
  const input = units(prices.input_price);
  const output = units(prices.output_price);
  if (input === null || output === null) return null;
  const cached = BigInt(usage.cached_tokens);
  const written = BigInt(usage.cache_write_tokens);
  const plain = BigInt(Math.max(0, usage.input_tokens - usage.cached_tokens - usage.cache_write_tokens));
  const read = units(prices.cache_read_price) ?? input;
  const write = units(prices.cache_write_price) ?? input;
  const total = plain * input + cached * read + written * write + BigInt(usage.output_tokens) * output;
  return decimal(total / 1_000_000n);
}
