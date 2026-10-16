const kept = new Map<string, HTMLImageElement>();

export function preloadPictures(urls: (string | null | undefined)[]): void {
  for (const url of urls) {
    if (!url || kept.has(url)) continue;
    const img = new Image();
    img.src = url;
    kept.set(url, img);
  }
}
