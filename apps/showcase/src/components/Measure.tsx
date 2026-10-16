import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import tokens from "@logicplanes/kaeso-tokens/tokens.json";

interface Box {
  w: number;
  h: number;
  pl: number;
  pr: number;
  radius: string;
  font: string;
  gap: number;
}

const heightToken = (h: number) => {
  for (const [k, v] of Object.entries(tokens.control)) if (v.height === h) return `--k-control-${k}`;
  for (const [k, v] of Object.entries(tokens.row)) if (v === h) return `--k-row-${k.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}`;
  return `${h}px`;
};
const radiusToken = (r: string, h: number) => {
  if (parseFloat(r) >= h / 2) return "pill (--k-radius-pill)";
  for (const [k, v] of Object.entries(tokens.radius)) if (v === r) return `--k-radius-${k}`;
  return r;
};

export function Measure({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState<Box | null>(null);
  useLayoutEffect(() => {
    const el = ref.current?.firstElementChild as HTMLElement | null;
    if (!el) return;
    const measure = () => {
      const cs = getComputedStyle(el);
      setBox({
        w: el.offsetWidth,
        h: el.offsetHeight,
        pl: parseFloat(cs.paddingLeft),
        pr: parseFloat(cs.paddingRight),
        radius: cs.borderTopLeftRadius,
        font: `${cs.fontSize} / ${cs.lineHeight} · ${cs.fontWeight}`,
        gap: parseFloat(cs.columnGap) || 0,
      });
    };
    measure();
    document.fonts?.ready.then(measure);
  }, []);
  return (
    <div className="flex min-h-240 items-center justify-center rounded-well bg-panel p-48 shadow-ring">
      <div className="relative" style={{ zoom: 2 }}>
        <div ref={ref}>{children}</div>
        {box && (
          <>
            <div className="pointer-events-none absolute top-0 -right-16 flex h-full items-center" aria-hidden>
              <div className="h-full w-1 bg-danger" />
            </div>
            <div className="pointer-events-none absolute top-0 left-0 h-full bg-danger/15" style={{ width: box.pl }} aria-hidden />
            <div className="pointer-events-none absolute top-0 right-0 h-full bg-danger/15" style={{ width: box.pr }} aria-hidden />
          </>
        )}
      </div>
      {box && (
        <dl className="ml-48 grid grid-cols-[auto_auto] gap-x-16 gap-y-6 text-small">
          <dt className="text-fg-secondary">Height</dt>
          <dd className="mono text-fg">
            {box.h}px · {heightToken(box.h)}
          </dd>
          <dt className="text-fg-secondary">Padding</dt>
          <dd className="mono text-fg">
            {box.pl}px · {box.pr}px
          </dd>
          <dt className="text-fg-secondary">Gap</dt>
          <dd className="mono text-fg">{box.gap}px</dd>
          <dt className="text-fg-secondary">Radius</dt>
          <dd className="mono text-fg">{radiusToken(box.radius, box.h)}</dd>
          <dt className="text-fg-secondary">Type</dt>
          <dd className="mono text-fg">{box.font}</dd>
          <dt className="text-fg-secondary">Width</dt>
          <dd className="mono text-fg">{box.w}px</dd>
        </dl>
      )}
    </div>
  );
}
