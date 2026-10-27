export function numberWidth(count: number): string {
  const digits = String(count).length;
  return digits <= 2 ? "w-16" : digits === 3 ? "w-24" : digits === 4 ? "w-32" : "w-40";
}
