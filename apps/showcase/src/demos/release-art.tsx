import { ReleaseArt, RELEASE_ART_STYLES } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ReleaseArt",
  description: "Flat geometric artwork for a release, from a stored spec or a seed: the same picture every time, a new one for every release.",
  group: "Components",
  entry: "release-art",
  import: "ReleaseArt",
  a11y: ["The artwork is an image with a label; the title drawn on it is part of the picture, so say it in the label too."],
};
export const examples = [{ id: "whats-new", title: "What's new", fn: "WhatsNew" }];
export function WhatsNew() {
  return (
    <div className="w-full max-w-640">
      <ReleaseArt
        seed="0.3.0"
        art={{ version: 1, seed: 2886621037, style: "stripes", palette: 6 }}
        label="Kaeso 0.3.0"
        eyebrow="What's new in Kaeso"
        title="0.3.0"
      />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Every release draws its own">
        <div className="grid w-full grid-cols-1 gap-16 sm:grid-cols-3">
          {["0.2.1", "0.3.0", "1.0.0"].map((v) => (
            <ReleaseArt key={v} seed={v} label={`Kaeso ${v}`} title={v} />
          ))}
        </div>
      </StateSection>
      <StateSection title="Eight styles">
        <div className="grid w-full grid-cols-1 gap-16 sm:grid-cols-2">
          {RELEASE_ART_STYLES.map((style, i) => (
            <ReleaseArt
              key={style}
              seed={style}
              art={{ version: 1, seed: 1000 + i, style, palette: (i * 5) % 12 }}
              label={`Kaeso 0.${i + 1}.0`}
              title={`0.${i + 1}.0`}
            />
          ))}
        </div>
      </StateSection>
      <StateSection title="Wide, without text">
        <div className="w-full max-w-640">
          <ReleaseArt seed="0.4.0" label="Artwork for Kaeso 0.4.0" shape="wide" />
        </div>
      </StateSection>
    </>
  );
}
