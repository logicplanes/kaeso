import { tickIncrement, ticks } from "d3-array";

type Pair = readonly [number, number];
type NumberLike = number | { valueOf(): number };

export interface LinearScale {
  (value: NumberLike): number;
  domain(): [number, number];
  domain(next: Pair): LinearScale;
  range(): [number, number];
  range(next: Pair): LinearScale;
  nice(count?: number): LinearScale;
  ticks(count?: number): number[];
  invert(value: number): number;
  copy(): LinearScale;
}

function map(from: Pair, to: Pair): (value: NumberLike) => number {
  const [a, b] = from[1] < from[0] ? [from[1], from[0]] : from;
  const [c, d] = from[1] < from[0] ? [to[1], to[0]] : to;
  const span = b - a;
  return (value) => {
    const t = span ? (+value - a) / span : Number.isNaN(span) ? NaN : 0.5;
    return c * (1 - t) + d * t;
  };
}

function niceDomain([lo, hi]: Pair, count: number): [number, number] {
  const reversed = hi < lo;
  let start = reversed ? hi : lo;
  let stop = reversed ? lo : hi;
  let previous: number | undefined;
  for (let i = 0; i < 10; i++) {
    const step = tickIncrement(start, stop, count);
    if (step === previous) return reversed ? [stop, start] : [start, stop];
    if (step > 0) {
      start = Math.floor(start / step) * step;
      stop = Math.ceil(stop / step) * step;
    } else if (step < 0) {
      start = Math.ceil(start * step) / step;
      stop = Math.floor(stop * step) / step;
    } else break;
    previous = step;
  }
  return [lo, hi];
}

export function scaleLinear(domain: Pair = [0, 1], range: Pair = [0, 1]): LinearScale {
  let d: [number, number] = [domain[0], domain[1]];
  let r: [number, number] = [range[0], range[1]];
  let forward = map(d, r);
  let backward = map(r, d);
  const rescale = () => {
    forward = map(d, r);
    backward = map(r, d);
  };
  const scale = ((value: NumberLike) => forward(value)) as LinearScale;
  function domainOf(): [number, number];
  function domainOf(next: Pair): LinearScale;
  function domainOf(next?: Pair) {
    if (!next) return [d[0], d[1]];
    d = [+next[0], +next[1]];
    rescale();
    return scale;
  }
  function rangeOf(): [number, number];
  function rangeOf(next: Pair): LinearScale;
  function rangeOf(next?: Pair) {
    if (!next) return [r[0], r[1]];
    r = [+next[0], +next[1]];
    rescale();
    return scale;
  }
  scale.domain = domainOf;
  scale.range = rangeOf;
  scale.nice = (count = 10) => domainOf(niceDomain(d, count));
  scale.ticks = (count = 10) => ticks(d[0], d[1], count);
  scale.invert = (value) => backward(value);
  scale.copy = () => scaleLinear(d, r);
  return scale;
}
