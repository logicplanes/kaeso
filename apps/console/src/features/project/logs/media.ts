import { formatDuration, formatNumber } from "@logicplanes/kaeso-ui/format";
import { t } from "~/i18n";

interface Units {
  unit: string;
  input: number;
  output: number;
}

export function mediaLabel(unit: string): string {
  if (unit === "character") return t("logs.inspector.speech", "Speech");
  if (unit === "millisecond") return t("logs.inspector.audio", "Audio");
  if (unit === "video") return t("logs.inspector.video", "Video");
  return t("logs.inspector.images", "Images");
}

export function mediaCount(m: Units): string {
  if (m.unit === "character") {
    const characters = t("logs.inspector.characters", "{count} characters", { count: formatNumber(m.input) });
    return m.output ? t("logs.inspector.charactersMade", "{characters} · {duration} made", { characters, duration: formatDuration(m.output) }) : characters;
  }
  if (m.unit === "millisecond") {
    return m.output
      ? t("logs.inspector.audioMade", "{duration} made", { duration: formatDuration(m.output) })
      : t("logs.inspector.audioSent", "{duration} of audio sent", { duration: formatDuration(m.input) });
  }
  if (m.unit === "video") {
    const videos = t("logs.inspector.videos", "{count, plural, one {# video} other {# videos}}", { count: m.input });
    return m.output ? t("logs.inspector.videosMade", "{videos} · {duration}", { videos, duration: formatDuration(m.output) }) : videos;
  }
  return t("logs.inspector.imagesCount", "{input} sent · {output} made", { input: formatNumber(m.input), output: formatNumber(m.output) });
}

export function mediaShort(m: Units): string {
  if (m.unit === "character") return t("logs.inspector.characters", "{count} characters", { count: formatNumber(m.input) });
  if (m.unit === "millisecond") return m.output || m.input ? formatDuration(m.output || m.input) : "—";
  if (m.unit === "video") {
    const videos = t("logs.inspector.videos", "{count, plural, one {# video} other {# videos}}", { count: m.input });
    return m.output ? t("logs.inspector.videosMade", "{videos} · {duration}", { videos, duration: formatDuration(m.output) }) : videos;
  }
  return m.output
    ? t("logs.media.images", "{count, plural, one {# image} other {# images}}", { count: m.output })
    : t("logs.media.imagesSent", "{count, plural, one {# image} other {# images}} sent", { count: m.input });
}
