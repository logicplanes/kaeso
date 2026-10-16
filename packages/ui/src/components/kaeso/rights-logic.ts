export interface RightChange {
  code: string;
  granted: boolean;
}

interface Node {
  code: string;
  granted: boolean;
  requires?: string[] | undefined;
  disabled?: boolean | undefined;
}

export function rightsCascade(rights: readonly Node[], code: string, granted: boolean): RightChange[] {
  const byCode = new Map(rights.map((r) => [r.code, r]));
  const dependents = new Map<string, string[]>();
  for (const r of rights) for (const req of r.requires ?? []) dependents.set(req, [...(dependents.get(req) ?? []), r.code]);
  const out: RightChange[] = [{ code, granted }];
  const seen = new Set([code]);
  const queue = [code];
  while (queue.length) {
    const next = queue.shift() as string;
    const linked = granted ? (byCode.get(next)?.requires ?? []) : (dependents.get(next) ?? []);
    for (const c of linked) {
      if (seen.has(c)) continue;
      seen.add(c);
      const r = byCode.get(c);
      if (!r || r.disabled) continue;
      queue.push(c);
      if (r.granted !== granted) out.push({ code: c, granted });
    }
  }
  return out;
}

export function groupCascade(all: readonly Node[], group: readonly Node[], granted: boolean): RightChange[] {
  const state = new Map(all.map((r) => [r.code, r.granted]));
  const out = new Map<string, boolean>();
  for (const r of group) {
    if (r.disabled || state.get(r.code) === granted) continue;
    const current = all.map((n) => ({ ...n, granted: out.get(n.code) ?? n.granted }));
    for (const c of rightsCascade(current, r.code, granted)) {
      out.set(c.code, c.granted);
      state.set(c.code, c.granted);
    }
  }
  return [...out].map(([code, g]) => ({ code, granted: g }));
}
