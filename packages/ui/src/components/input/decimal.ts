function split(v: string): { neg: boolean; int: string; frac: string } {
  const m = /^(-)?(\d*)(?:\.(\d*))?$/.exec(v.trim());
  if (!m) return { neg: false, int: "0", frac: "" };
  return { neg: m[1] === "-", int: (m[2] || "0").replace(/^0+(?=\d)/, ""), frac: m[3] ?? "" };
}

export function isDecimal(v: string): boolean {
  return /^-?\d*(?:\.\d*)?$/.test(v.trim());
}

export function addDecimal(a: string, b: string): string {
  const x = split(a || "0");
  const y = split(b || "0");
  const scale = Math.max(x.frac.length, y.frac.length);
  const toBig = (p: { neg: boolean; int: string; frac: string }) => BigInt((p.neg ? "-" : "") + p.int + p.frac.padEnd(scale, "0"));
  const sum = toBig(x) + toBig(y);
  const neg = sum < 0n;
  const digits = (neg ? -sum : sum).toString().padStart(scale + 1, "0");
  const int = digits.slice(0, digits.length - scale) || "0";
  const frac = scale ? digits.slice(-scale) : "";
  return `${neg ? "-" : ""}${int}${frac ? `.${frac}` : ""}`;
}

export function compareDecimal(a: string, b: string): number {
  const d = addDecimal(a, b.startsWith("-") ? b.slice(1) : `-${b}`);
  if (d.startsWith("-") && /[1-9]/.test(d)) return -1;
  return /[1-9]/.test(d) ? 1 : 0;
}

export interface DecimalSeparators {
  group: string;
  decimal: string;
}

const ENGLISH: DecimalSeparators = { group: ",", decimal: "." };

export function decimalSeparators(locale: string): DecimalSeparators {
  if (locale === "en") return ENGLISH;
  const parts = new Intl.NumberFormat(locale).formatToParts(1234.5);
  return { group: parts.find((p) => p.type === "group")?.value ?? ",", decimal: parts.find((p) => p.type === "decimal")?.value ?? "." };
}

export function groupDecimal(v: string, seps: DecimalSeparators = ENGLISH): string {
  const p = split(v);
  if (!isDecimal(v) || v.trim() === "") return v;
  return `${p.neg ? "-" : ""}${p.int.replace(/\B(?=(\d{3})+(?!\d))/g, seps.group)}${v.includes(".") ? `${seps.decimal}${p.frac}` : ""}`;
}

export function localizeDecimal(v: string, seps: DecimalSeparators = ENGLISH): string {
  return seps.decimal === "." ? v : v.replace(".", seps.decimal);
}

export function ungroup(v: string, seps: DecimalSeparators = ENGLISH): string {
  const out = seps.group === "." ? v : v.split(seps.group).join("");
  return seps.decimal === "." ? out : out.split(seps.decimal).join(".");
}
