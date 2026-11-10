import { useEffect, useState } from "react";
import { MediaView, Text } from "@logicplanes/kaeso-ui";
import { kindLabel, leftOutLabel, mediaDetail } from "./left-out";
import { cleanName, fileName, shownAs, standard, toBlob, type Kept, type LoggedMedia } from "./logged-media";

// Audio and video play from a blob: address and an image is drawn from a data: URL, the two the console image's
// content security policy allows (`media-src 'self' blob:`, `img-src 'self' data:`). Every download is a blob of
// the type chosen here, so a document inside logged content is never opened as a page.
function useBlobUrl(media: Kept, type: string): string | undefined {
  const [url, setUrl] = useState<string>();
  useEffect(() => {
    const made = URL.createObjectURL(toBlob(media, type));
    let live = true;
    void Promise.resolve().then(() => live && setUrl(made));
    return () => {
      live = false;
      URL.revokeObjectURL(made);
    };
  }, [media, type]);
  return url;
}

export function KeptMedia({ media, name, index = 0 }: { media: Kept; name?: string | undefined; index?: number }) {
  const shown = shownAs(media);
  const blob = useBlobUrl(media, shown.type);
  if (!blob) return null;
  return (
    <MediaView
      kind={shown.kind}
      src={shown.kind === "image" ? `data:${shown.type};base64,${standard(media.base64)}` : blob}
      download={blob}
      label={cleanName(name) || kindLabel(shown.kind)}
      detail={mediaDetail({ mediaType: shown.kind === "file" ? media.mediaType : shown.type, bytes: media.bytes })}
      filename={fileName(shown, name, index)}
      className="mx-12"
    />
  );
}

export function MediaList({ media }: { media: LoggedMedia[] }) {
  if (media.length === 0) return null;
  return (
    <div className="flex flex-col gap-12">
      {media.map((m, i) =>
        "kept" in m ? (
          <KeptMedia key={i} media={m.kept} index={i} />
        ) : (
          <Text key={i} variant="small" tone="secondary" className="mx-12">
            {leftOutLabel(m.leftOut)}
          </Text>
        ),
      )}
    </div>
  );
}
