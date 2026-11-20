// Without a canvas, Inter's widest Latin run: 6.9px a character at 13px (apps/console/AGENTS.md), so 7.4px at the cells'
// 14px and 6.4px at the headers' 12px; CJK takes a full em.
const CELL = { font: "14px", latin: 7.4, wide: 14 };
const HEADER = { font: "500 12px", latin: 6.4, wide: 12 };
const CELL_PADDING = 24;
// DataTable keeps a sortable header's 12px arrow and its 4px gap in place while it's hidden, so the label doesn't move.
const SORT_ARROW = 16;

const isWide = (code: number) => (code >= 0x2e80 && code <= 0xa4cf) || (code >= 0xac00 && code <= 0xd7a3) || (code >= 0xff00 && code <= 0xffef);

const canvases = new Map<string, OffscreenCanvasRenderingContext2D | null>();

function measurer(font: string): OffscreenCanvasRenderingContext2D | null {
  let canvas = canvases.get(font);
  if (canvas === undefined) {
    canvas = typeof OffscreenCanvas === "undefined" ? null : new OffscreenCanvas(1, 1).getContext("2d");
    if (canvas) canvas.font = `${font} ${getComputedStyle(document.body).fontFamily}`;
    canvases.set(font, canvas);
  }
  return canvas;
}

function measure(text: string, { font, latin, wide }: typeof CELL): number {
  const measured = measurer(font)?.measureText(text).width;
  if (measured !== undefined) return measured;
  let width = 0;
  for (const ch of text) width += isWide(ch.charCodeAt(0)) ? wide : latin;
  return width;
}

export const textWidth = (text: string): number => measure(text, CELL);

const onGrid = (width: number) => Math.ceil(width / 8) * 8;

export function cellWidth(texts: Iterable<string>, { min, max }: { min: number; max: number }): number {
  let widest = 0;
  for (const text of texts) widest = Math.max(widest, textWidth(text));
  return Math.min(max, Math.max(min, onGrid(widest + CELL_PADDING)));
}

export function headerWidth(header: string, min: number): number {
  return Math.max(min, onGrid(measure(header, HEADER) + SORT_ARROW + CELL_PADDING));
}
