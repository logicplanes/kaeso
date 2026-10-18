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
