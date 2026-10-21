import { utcTicks } from "d3-time";

export type WallClock = (d: Date) => number;

const clocks = new Map<string, WallClock>();

export function wallClock(timeZone: string | undefined): WallClock {
  if (!timeZone) return (d) => d.getTime() - d.getTimezoneOffset() * 60_000;
  if (timeZone === "UTC") return (d) => d.getTime();
  let clock = clocks.get(timeZone);
  if (!clock) {
    const f = new Intl.DateTimeFormat("en-US", {
      timeZone,
      hourCycle: "h23",
      year: "numeric",
      month: "numeric",
      day: "numeric",
      hour: "numeric",
      minute: "numeric",
      second: "numeric",
    });
    clock = (d) => {
      const p = Object.fromEntries(f.formatToParts(d).map((x) => [x.type, Number(x.value)]));
      return Date.UTC(p.year ?? 1970, (p.month ?? 1) - 1, p.day ?? 1, p.hour ?? 0, p.minute ?? 0, p.second ?? 0) + d.getUTCMilliseconds();
    };
    clocks.set(timeZone, clock);
  }
  return clock;
}

export function fromWall(wall: number, clock: WallClock): Date {
  const guess = wall - (clock(new Date(wall)) - wall);
  return new Date(wall - (clock(new Date(guess)) - guess));
}

export function zoneTicks(from: Date, to: Date, count: number, clock: WallClock): Date[] {
  return utcTicks(new Date(clock(from)), new Date(clock(to)), count).flatMap((w) => {
    const d = fromWall(w.getTime(), clock);
    // A wall time skipped when clocks spring forward has no instant; it would land on the next tick.
    return clock(d) === w.getTime() ? [d] : [];
  });
}

export const wallDay = (d: Date, clock: WallClock) => new Date(clock(d)).toISOString().slice(0, 10);
