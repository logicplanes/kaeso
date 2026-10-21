import { ReleaseArt } from "@logicplanes/kaeso-ui/release-art";
import { releaseArtUrl } from "~/site/update-service";

export default function ReleaseArtwork({ version, label }: { version: string; label: string }) {
  return <ReleaseArt seed={version} src={releaseArtUrl(version)} label={label} shape="wide" />;
}
