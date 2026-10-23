// Without a canvas, Inter's widest Latin run: 6.9px a character at 13px (apps/console/AGENTS.md), so 7.4px at the cells'
// 14px; CJK takes a full em.
const LATIN = 7.4;
const WIDE = 14;
const CELL_FONT = "14px";
const CELL_PADDING = 24;

const isWide = (code: number) => (code >= 0x2e80 && code <= 0xa4cf) || (code >= 0xac00 && code <= 0xd7a3) || (code >= 0xff00 && code <= 0xffef);

let canvas: OffscreenCanvasRenderingContext2D | null | undefined;

function measurer(): OffscreenCanvasRenderingContext2D | null {
  if (canvas === undefined) {
    canvas = typeof OffscreenCanvas === "undefined" ? null : new OffscreenCanvas(1, 1).getContext("2d");
    if (canvas) canvas.font = `${CELL_FONT} ${getComputedStyle(document.body).fontFamily}`;
  }
  return canvas;
}

export function textWidth(text: string): number {
  const measured = measurer()?.measureText(text).width;
  if (measured !== undefined) return measured;
  let width = 0;
  for (const ch of text) width += isWide(ch.charCodeAt(0)) ? WIDE : LATIN;
  return width;
}

export function cellWidth(texts: Iterable<string>, { min, max }: { min: number; max: number }): number {
  let widest = 0;
  for (const text of texts) widest = Math.max(widest, textWidth(text));
  return Math.min(max, Math.max(min, Math.ceil((widest + CELL_PADDING) / 8) * 8));
}
