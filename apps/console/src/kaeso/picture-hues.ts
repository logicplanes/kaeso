import { useEffect, useState } from "react";

const known = new Map<string, number | null>();

export function hueOf(pixels: Uint8ClampedArray): number | null {
  const px = new DataView(pixels.buffer, pixels.byteOffset, pixels.byteLength);
  let [r, g, b, weight] = [0, 0, 0, 0];
  for (let i = 0; i + 3 < px.byteLength; i += 4) {
    const a = px.getUint8(i + 3) / 255;
    r += px.getUint8(i) * a;
    g += px.getUint8(i + 1) * a;
    b += px.getUint8(i + 2) * a;
    weight += a;
  }
  if (!weight) return null;
  [r, g, b] = [r, g, b].map((v) => v / weight / 255) as [number, number, number];
  const max = Math.max(r, g, b);
  const spread = max - Math.min(r, g, b);
  if (spread < 0.12) return null;
  const h = max === r ? ((g - b) / spread) % 6 : max === g ? (b - r) / spread + 2 : (r - g) / spread + 4;
  return Math.round((h * 60 + 360) % 360);
}

const SIDE = 16;

function readHue(src: string): Promise<number | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = SIDE;
      canvas.height = SIDE;
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      if (!ctx) return resolve(null);
      ctx.drawImage(img, 0, 0, SIDE, SIDE);
      resolve(hueOf(ctx.getImageData(0, 0, SIDE, SIDE).data));
    };
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

export function usePictureHues(pictures: { id: string; src: string | null | undefined }[]): ReadonlyMap<string, number> {
  const wanted = pictures.flatMap((p) => (p.src ? [p.src] : []));
  const key = wanted.join("\n");
  const [, setRead] = useState(0);
  useEffect(() => {
    let live = true;
    const unread = key.split("\n").filter((src) => src && !known.has(src));
    void Promise.all(unread.map(async (src) => known.set(src, await readHue(src)))).then(() => {
      if (live && unread.length) setRead((n) => n + 1);
    });
    return () => {
      live = false;
    };
  }, [key]);
  const hues = new Map<string, number>();
  for (const p of pictures) {
    const hue = p.src ? known.get(p.src) : null;
    if (hue !== null && hue !== undefined) hues.set(p.id, hue);
  }
  return hues;
}
