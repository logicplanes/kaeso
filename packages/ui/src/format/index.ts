import { formatMessage, type MessageValues } from "../i18n/message.js";

let current = "en";
let words: Readonly<Record<string, string>> = {};
let zone: string | undefined;

export function setFormatLocale(locale: string, messages: Readonly<Record<string, string>> = {}): void {
  current = locale;
  words = messages;
}

const t = (key: string, english: string, values?: MessageValues) => formatMessage(words[key] ?? english, values, current);

export function formatLocale(): string {
  return current;
}

export function setFormatTimeZone(timeZone: string | undefined): void {
  if (!timeZone) {
    zone = undefined;
    return;
  }
  try {
    new Intl.DateTimeFormat("en", { timeZone });
    zone = timeZone;
  } catch {
    zone = undefined;
  }
}

export function formatTimeZone(): string | undefined {
  return zone;
}

const inZone = (): { timeZone?: string } => (zone ? { timeZone: zone } : {});

const numberLocale = () => (current === "en" ? "en-US" : current);

export function formatNumber(value: number, options: Intl.NumberFormatOptions = {}): string {
  return new Intl.NumberFormat(numberLocale(), { maximumFractionDigits: 2, ...options }).format(value);
}

const COMPACT = { maximumFractionDigits: 1, maximumSignificantDigits: 3, roundingPriority: "morePrecision" } as const;

export function formatTokens(value: number): string {
  if (Math.abs(value) < 1000) return formatNumber(value, { maximumFractionDigits: 0 });
  const compact = new Intl.NumberFormat(numberLocale(), { notation: "compact", ...COMPACT }).format(value);
  // German and Chinese have no short form below a million and ten thousand, so 793,640 would stand in full beside 2.89 million.
  if (/\p{L}/u.test(compact)) return compact;
  return t("format.thousands", "{value}K", { value: formatNumber(value / 1000, COMPACT) });
}

/** A model's context or output limit as models are named: a multiple of 1,024 in binary thousands ("128K" for 131,072, "1M" for 1,048,576), anything else in decimal ones ("128K" for 128,000, "200K"). */
export function formatContext(tokens: number): string {
  const binary = tokens % 1000 !== 0 && tokens % 1024 === 0;
  const k = binary ? tokens / 1024 : tokens / 1000;
  const unit = binary ? 1024 : 1000;
  if (tokens < 1000) return formatNumber(tokens, { maximumFractionDigits: 0 });
  if (k >= unit) return `${formatNumber(k / unit, { maximumFractionDigits: 1 })}M`;
  return `${formatNumber(k, { maximumFractionDigits: k < 10 ? 1 : 0 })}K`;
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = bytes;
  let i = 0;
  while (Math.abs(v) >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  const digits = i === 0 || v >= 100 ? 0 : 1;
  return `${formatNumber(v, { minimumFractionDigits: digits, maximumFractionDigits: digits, useGrouping: false })} ${units[i]}`;
}

export function formatPercent(ratio: number, digits = 1): string {
  return new Intl.NumberFormat(numberLocale(), { style: "percent", maximumFractionDigits: digits }).format(ratio);
}

function groupThousands(int: string): string {
  return int.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

export function formatUSD(amount: string, options: { significant?: number; decimals?: number } = {}): string {
  const text = writeUSD(amount, options);
  if (options.decimals === undefined || !/^\s*-?\d*\.?\d*[1-9]/.test(amount) || /[1-9]/.test(text)) return localUSD(text);
  const smallest = localUSD(`$${options.decimals ? `0.${"1".padStart(options.decimals, "0")}` : "1"}`);
  return /^\s*-/.test(amount) ? `>\u00a0−${smallest}` : `<\u00a0${smallest}`;
}

/** A price per million tokens, exactly as set: at least two decimals and up to six ("$0.075", "$4.00", "$1.25"). */
export function formatPrice(amount: string): string {
  const frac = /\.(\d*?)0*\s*$/.exec(amount)?.[1] ?? "";
  return formatUSD(amount, { decimals: Math.min(6, Math.max(2, frac.length)) });
}

function localUSD(text: string): string {
  const m = /^(−?)\$([\d,]+)(?:\.(\d+))?$/.exec(text);
  if (current === "en" || current.startsWith("zh") || !m) return text;
  const parts = new Intl.NumberFormat(numberLocale(), { style: "currency", currency: "USD" }).formatToParts(1000.5);
  const part = (type: string) => parts.find((p) => p.type === type)?.value;
  const at = parts.findIndex((p) => p.type === "currency");
  const before = at < parts.findIndex((p) => p.type === "integer");
  const gap = parts[before ? at + 1 : at - 1];
  const space = gap?.type === "literal" ? gap.value : "";
  const number = (m[2] ?? "").replace(/,/g, part("group") ?? ",") + (m[3] ? `${part("decimal") ?? "."}${m[3]}` : "");
  const symbol = part("currency") ?? "$";
  return before ? `${m[1]}${symbol}${space}${number}` : `${m[1]}${number}${space}${symbol}`;
}

function writeUSD(amount: string, options: { significant?: number; decimals?: number }): string {
  const significant = options.significant ?? 3;
  const match = /^\s*(-)?(\d*)(?:\.(\d*))?\s*$/.exec(amount);
  if (!match) return amount;
  const negative = match[1] === "-";
  const intPart = (match[2] ?? "").replace(/^0+(?=\d)/, "") || "0";
  const frac = match[3] ?? "";
  const sign = negative ? "−" : "";
  if (options.decimals !== undefined) {
    const n = options.decimals;
    const digits = (intPart + frac.padEnd(n + 1, "0").slice(0, n + 1)).replace(/^0+(?=\d)/, "");
    let scaled = BigInt(digits.slice(0, -1) || "0");
    if (Number(digits.slice(-1)) >= 5) scaled += 1n;
    const str = scaled.toString().padStart(n + 1, "0");
    const whole = str.slice(0, str.length - n) || "0";
    const isZero = scaled === 0n;
    return `${negative && !isZero ? sign : ""}$${groupThousands(whole)}${n ? `.${str.slice(-n)}` : ""}`;
  }
  if (intPart === "0" && /^0{2}/.test(frac.padEnd(2, "0")) && /[1-9]/.test(frac)) {
    let lead = frac.match(/^0*/)?.[0].length ?? 0;
    const take = frac.slice(lead, lead + significant + 1).padEnd(significant + 1, "0");
    let kept = BigInt(take.slice(0, significant));
    if (Number(take[significant]) >= 5) kept += 1n;
    let digits = kept.toString().padStart(significant, "0");
    if (digits.length > significant) {
      lead -= 1;
      digits = digits.slice(0, significant);
    }
    if (lead < 2) return `${sign}$0.${"0".repeat(Math.max(0, lead))}${digits}`.replace(/^(−?\$0\.\d{2})\d*$/, "$1");
    return `${sign}$0.${"0".repeat(lead)}${digits}`;
  }
  const cents = frac.padEnd(3, "0");
  let whole = BigInt(intPart);
  let two = Number(cents.slice(0, 2));
  if (Number(cents[2]) >= 5) two += 1;
  if (two === 100) {
    two = 0;
    whole += 1n;
  }
  const isZero = whole === 0n && two === 0;
  return `${negative && !isZero ? sign : ""}$${groupThousands(whole.toString())}.${String(two).padStart(2, "0")}`;
}

export function formatDuration(ms: number, options: { precise?: boolean | undefined } = {}): string {
  const whole = Math.round(ms);
  if (whole < 1000) return t("format.milliseconds", "{ms} ms", { ms: String(whole) });
  const places = options.precise ? 2 : 1;
  const secondsTo = (digits: number) => Math.round(ms / 10 ** (3 - digits)) / 10 ** digits;
  const seconds = (digits: number) =>
    t("format.seconds", "{s} s", {
      s: formatNumber(secondsTo(digits), { minimumFractionDigits: options.precise ? digits : 0, maximumFractionDigits: digits }),
    });
  if (secondsTo(places) < 10) return seconds(places);
  if (secondsTo(places - 1) < 60) return seconds(places - 1);
  const total = secondsTo(0);
  if (total < 3600) {
    const min = Math.floor(total / 60);
    const s = total % 60;
    return s ? t("format.minutesSeconds", "{min} min {s} s", { min: String(min), s: String(s) }) : t("format.minutes", "{min} min", { min: String(min) });
  }
  const minutes = Math.round(ms / 60_000);
  return t("format.hoursMinutes", "{h} h {min} min", { h: String(Math.floor(minutes / 60)), min: String(minutes % 60) });
}

type DateInput = Date | string | number;
const toDate = (d: DateInput) => (d instanceof Date ? d : new Date(d));

const dateFormats = new Map<string, (d: Date) => string>();

/** A cached date formatter in the format locale. English writes the day first with three-letter months ("20 Sep 2026", "Mon 21 Sep"), the console's English everywhere. */
export function dateFormatter(options: Intl.DateTimeFormatOptions, locale: string = current): (value: DateInput) => string {
  // A cached formatter keeps the zone it was made in, so the viewer's zone is part of the key.
  const zoned = options.timeZone ? options : { ...options, timeZone: new Intl.DateTimeFormat().resolvedOptions().timeZone };
  const key = `${locale}|${JSON.stringify(zoned)}`;
  let format = dateFormats.get(key);
  if (!format) {
    if (locale === "en") {
      const day = new Intl.DateTimeFormat("en-GB", zoned);
      const month = zoned.month === "short" ? new Intl.DateTimeFormat("en-US", { month: "short", timeZone: zoned.timeZone }) : null;
      format = (d) =>
        day
          .formatToParts(d)
          .map((p) => (p.type === "month" && month ? month.format(d) : p.type === "weekday" ? p.value.replace(/,$/, "") : p.value))
          .join("");
    } else {
      const f = new Intl.DateTimeFormat(locale, zoned);
      format = (d) => f.format(d);
    }
    dateFormats.set(key, format);
  }
  return (value) => format(toDate(value));
}

export function formatDate(value: DateInput, options: { time?: boolean; utc?: boolean; seconds?: boolean } = {}): string {
  const f = dateFormatter({
    day: "numeric",
    month: "short",
    year: "numeric",
    ...(options.time ? { hour: "2-digit", minute: "2-digit", ...(options.seconds ? { second: "2-digit" } : {}), hourCycle: "h23" } : {}),
    ...(options.utc ? { timeZone: "UTC" } : inZone()),
  });
  return `${f(value)}${options.utc ? " UTC" : ""}`;
}

export function formatDateRange(from: DateInput, to: DateInput): string {
  const a = toDate(from);
  const b = toDate(to);
  const f = dateFormatter({ day: "numeric", month: "short" });
  const withYear = dateFormatter({ day: "numeric", month: "short", year: "numeric" });
  if (a.getFullYear() !== b.getFullYear()) return `${withYear(a)} – ${withYear(b)}`;
  const start = f(a);
  return a.toDateString() === b.toDateString() ? start : `${start} – ${f(b)}`;
}

const pad2 = (n: number) => String(n).padStart(2, "0");

export function formatISODate(value: DateInput, options: { utc?: boolean | undefined } = {}): string {
  const d = toDate(value);
  if (options.utc) return d.toISOString().slice(0, 10);
  return `${String(d.getFullYear()).padStart(4, "0")}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

export function parseISODate(text: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text.trim());
  if (!m) return null;
  const [y, mo, day] = [Number(m[1]), Number(m[2]) - 1, Number(m[3])];
  const d = new Date(2000, 0, 1);
  d.setFullYear(y, mo, day);
  return d.getFullYear() === y && d.getMonth() === mo && d.getDate() === day ? d : null;
}

export function formatRelative(value: DateInput, now: DateInput = Date.now()): string {
  const d = toDate(value);
  const diff = toDate(now).getTime() - d.getTime();
  const abs = Math.abs(diff);
  const future = diff < 0;
  if (abs < 45_000) return t("format.justNow", "just now");
  if (abs < 3_600_000) {
    const count = Math.round(abs / 60_000);
    return future ? t("format.inMinutes", "in {count} min", { count }) : t("format.minutesAgo", "{count} min ago", { count });
  }
  if (abs < 86_400_000) {
    const count = Math.round(abs / 3_600_000);
    return future ? t("format.inHours", "in {count} h", { count }) : t("format.hoursAgo", "{count} h ago", { count });
  }
  if (abs < 172_800_000 && !future) return t("format.yesterday", "yesterday");
  if (abs < 7 * 86_400_000) {
    const count = Math.round(abs / 86_400_000);
    return future
      ? t("format.inDays", "{count, plural, one {in # day} other {in # days}}", { count })
      : t("format.daysAgo", "{count, plural, one {# day ago} other {# days ago}}", { count });
  }
  const year = dateFormatter({ year: "numeric", ...inZone() }, "en");
  return dateFormatter({
    day: "numeric",
    month: "short",
    ...inZone(),
    ...(year(d) !== year(toDate(now)) ? { year: "numeric" } : {}),
  })(d);
}
