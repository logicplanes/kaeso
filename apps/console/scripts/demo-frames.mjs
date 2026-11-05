/** The README asks for a recording under 15 seconds, so a reader sees all of it before scrolling on. */
export const MAX_SECONDS = 15;

/** How long the GIF made from these frames plays, with the last frame held for `hold` seconds. */
export function seconds(frames, hold) {
  if (!frames.length) return 0;
  return frames.at(-1).time - frames[0].time + hold;
}

export function concatList(frames, hold) {
  if (!frames.length) throw new Error("demo: the screencast sent no frames");
  const last = frames.at(-1);
  const lines = ["ffconcat version 1.0"];
  frames.forEach((frame, i) => {
    const next = frames[i + 1];
    const seconds = next ? Math.max(0, next.time - frame.time) : hold;
    lines.push(`file '${frame.file}'`, `duration ${seconds.toFixed(3)}`);
  });
  lines.push(`file '${last.file}'`);
  return `${lines.join("\n")}\n`;
}

export function gifFilter({ width, fps }) {
  return [
    `fps=${fps},scale=${width}:-2:flags=lanczos,split[frames][copy]`,
    "[copy]palettegen=max_colors=256:stats_mode=full[palette]",
    "[frames][palette]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
  ].join(";");
}
