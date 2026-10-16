import { formatUSD } from "@logicplanes/kaeso-ui/format";

const isZero = (amount: string) => !/[1-9]/.test(amount);

export const requestUSD = (amount: string): string => (isZero(amount) ? formatUSD("0", { decimals: 0 }) : formatUSD(amount, { decimals: 6 }));

export function costUSD(amount: string): string {
  if (isZero(amount)) return formatUSD("0", { decimals: 0 });
  const [whole = "0", frac = ""] = amount.replace(/^-/, "").split(".");
  const lead = /^0*$/.test(whole) ? frac.length - frac.replace(/^0+/, "").length : 0;
  const places = Math.min(6, Math.max(2, /^0*$/.test(whole) ? lead + 3 : 2));
  const rounded = Math.abs(Number(amount)).toFixed(places);
  if (!/[1-9]/.test(rounded)) return `< ${formatUSD("0.000001", { decimals: 6 })}`;
  const decimals = Math.max(2, rounded.replace(/0+$/, "").split(".")[1]?.length ?? 0);
  return formatUSD(amount, { decimals });
}
