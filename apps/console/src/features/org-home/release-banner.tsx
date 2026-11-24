import { useQuery } from "@tanstack/react-query";
import { Announcement, Dashboard, ReleaseArt, isReleaseArtSpec } from "@logicplanes/kaeso-ui";
import { whatsNewQuery } from "~/api/queries/whats-new";
import { t } from "~/i18n";
import { ReleaseBannerPending, ReadWhatsNew } from "./release-banner-pending";

const firstSentence = (notes: string) =>
  notes
    .split("\n")
    .filter((l) => !l.trimStart().startsWith("#"))
    .map((l) =>
      l
        .replace(/^[>*\-\s]+/, "")
        .replace(/\*\*|__|`/g, "")
        .trim(),
    )
    .find((l) => l.length > 0) ?? "";

function useReleases() {
  const news = useQuery(whatsNewQuery);
  const all = news.data ? [...news.data.releases, news.data.running] : [];
  const unique = all.filter((r, i) => all.findIndex((x) => x.version === r.version) === i);
  return { loading: news.isPending, releases: unique.sort((a, b) => (b.published_at ?? "").localeCompare(a.published_at ?? "")), running: news.data?.running };
}

export function ReleaseBanner() {
  const { loading, releases, running } = useReleases();
  const latest = releases[0];
  if (loading) return <ReleaseBannerPending />;
  if (!latest) return null;
  const isNew = latest.version !== running?.version;
  return (
    <Dashboard aria-label={t("kaeso.home.news", "News")}>
      <Announcement
        className="col-span-full"
        eyebrow={isNew ? t("kaeso.home.newRelease", "New release") : t("kaeso.home.running", "Running now")}
        title={t("kaeso.home.releaseTitle", "Kaeso {version}", { version: latest.version })}
        description={firstSentence(latest.notes) || t("kaeso.home.noSummary", "There's no summary of this release to show here.")}
        actions={<ReadWhatsNew />}
        art={
          <ReleaseArt
            seed={latest.version}
            art={isReleaseArtSpec(latest.art) ? latest.art : undefined}
            label={t("shell.whatsNew.artLabel", "Kaeso {version}", { version: latest.version })}
            eyebrow={latest.prerelease ? t("shell.whatsNew.preEyebrow", "Kaeso pre-release") : t("shell.whatsNew.eyebrow", "Kaeso")}
            title={latest.version}
            flush
          />
        }
      />
    </Dashboard>
  );
}
