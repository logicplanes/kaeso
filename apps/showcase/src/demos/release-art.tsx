import { ReleaseArt, RELEASE_ART_FLAT_STYLES } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "ReleaseArt",
  description:
    "Artwork for a release, from a stored spec or a seed: its name in white on a dark ground lit by one bloom of light, the same picture every time and a new one for every release.",
  group: "Components",
  entry: "release-art",
  import: "ReleaseArt",
  a11y: ["The artwork is an image with a label; the title drawn on it is part of the picture, so say it in the label too."],
};
export const examples = [{ id: "whats-new", title: "What's new", fn: "WhatsNew" }];
export function WhatsNew() {
  return (
    <div className="w-full max-w-640">
      <ReleaseArt seed="3.1.0" art={{ version: 1, seed: 2886621037, style: "glow", palette: 1 }} label="Kaeso 3.1.0" eyebrow="Kaeso" title="3.1.0" />
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Every release glows in its own light">
        <div className="grid w-full grid-cols-1 gap-16 sm:grid-cols-3">
          {[0, 2, 3, 5, 7, 9].map((palette, i) => (
            <ReleaseArt
              key={palette}
              seed={`0.${8 + i}.0`}
              art={{ version: 1, seed: 4021 + i * 977, style: "glow", palette }}
              label={`Kaeso 0.${8 + i}.0`}
              eyebrow="Kaeso"
              title={`0.${8 + i}.0`}
            />
          ))}
        </div>
      </StateSection>
      <StateSection title="Drawn from the version when a release has no stored artwork">
        <div className="grid w-full grid-cols-1 gap-16 sm:grid-cols-3">
          {["0.7.6", "0.9.1", "1.0.0"].map((v) => (
            <ReleaseArt key={v} seed={v} label={`Kaeso ${v}`} eyebrow="Kaeso" title={v} />
          ))}
        </div>
      </StateSection>
      <StateSection title="A pre-release, and 320px">
        <div className="flex flex-col gap-16">
          <div className="w-320">
            <ReleaseArt
              seed="3.1.0-rc.1"
              art={{ version: 1, seed: 77, style: "glow", palette: 3 }}
              label="Kaeso 3.1.0-rc.1"
              eyebrow="Kaeso pre-release"
              title="3.1.0-rc.1"
            />
          </div>
          <div className="w-320">
            <ReleaseArt seed="3.1.0" art={{ version: 1, seed: 2886621037, style: "glow", palette: 1 }} label="Kaeso 3.1.0" eyebrow="Kaeso" title="3.1.0" />
          </div>
        </div>
      </StateSection>
      <StateSection title="The eight flat styles of earlier releases">
        <div className="grid w-full grid-cols-1 gap-16 sm:grid-cols-2">
          {RELEASE_ART_FLAT_STYLES.map((style, i) => (
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
      <StateSection title="A flat style at 320px, with a long title in German">
        <div className="w-320">
          <ReleaseArt
            seed="0.3.0"
            art={{ version: 1, seed: 2886621037, style: "stripes", palette: 6 }}
            label="Kaeso 0.3.0"
            eyebrow="Neuigkeiten in Kaeso"
            title="Veröffentlichung 0.3.0"
          />
        </div>
      </StateSection>
      <StateSection title="Wide, without text">
        <div className="w-full max-w-640">
          <ReleaseArt seed="0.4.0" label="Artwork for Kaeso 0.4.0" shape="wide" />
        </div>
      </StateSection>
      <StateSection title="A strip keeps its left clear for an announcement's words">
        <div className="grid w-full grid-cols-1 gap-16">
          {[1, 5, 9].map((palette, i) => (
            <ReleaseArt
              key={palette}
              seed={`0.${14 + i}.0`}
              art={{ version: 1, seed: 6113 + i * 977, style: "glow", palette }}
              label={`Kaeso 0.${14 + i}.0`}
              shape="strip"
            />
          ))}
        </div>
      </StateSection>
    </>
  );
}
