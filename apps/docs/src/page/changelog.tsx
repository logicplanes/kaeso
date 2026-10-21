import { Badge, Link, Separator, Text } from "@logicplanes/kaeso-ui";
import { formatDate, parseISODate } from "@logicplanes/kaeso-ui/format";
import { useLocation, useNavigate } from "@tanstack/react-router";
import { Fragment, Suspense, lazy, useEffect, type ReactNode } from "react";
import { useSiteLocale, useText } from "~/i18n/text";
import type { ChangelogContent, ChangelogEntry, ChangelogRelease } from "~/markdown/ast";
import { Markdown } from "~/markdown/render";

const ReleaseArtwork = lazy(() => import("./release-artwork"));

export function releaseDate(iso: string): string {
  const day = parseISODate(iso);
  return day ? formatDate(day) : iso;
}

export function ChangelogNote() {
  const t = useText();
  const locale = useSiteLocale();
  if (locale === "en") return null;
  return (
    <Text as="p" variant="body" tone="secondary" className="text-pretty">
      {t("changelog.english")}
    </Text>
  );
}

function Entry({ entry, lang }: { entry: ChangelogEntry; lang: string | undefined }) {
  const t = useText();
  return (
    <>
      {entry.intro.length > 0 && <Markdown blocks={entry.intro} lang={lang} />}
      {entry.sections.map((section) => (
        <div key={section.id} className="flex flex-col gap-12">
          <Text
            as="h3"
            id={section.id}
            variant="title-sm"
            weight="semibold"
            className="scroll-mt-24 text-balance wrap-break-word"
            {...(section.group ? {} : { lang })}
          >
            {section.group ? t(`changelog.${section.group}`) : section.name}
          </Text>
          {section.blocks.length > 0 && <Markdown blocks={section.blocks} lang={lang} />}
          {section.topics.map((topic) => (
            <div key={topic.id} className="mt-4 flex flex-col gap-8">
              <Text as="h4" id={topic.id} lang={lang} variant="body-lg" weight="semibold" className="scroll-mt-24 text-balance wrap-break-word">
                {topic.name}
              </Text>
              <Markdown blocks={topic.blocks} lang={lang} />
            </div>
          ))}
        </div>
      ))}
    </>
  );
}

function Heading({ id, title, children }: { id: string; title: string; children?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center gap-x-12 gap-y-4">
      <Text as="h2" id={id} variant="title" weight="semibold" className="scroll-mt-24 wrap-break-word">
        {title}
      </Text>
      {children}
    </div>
  );
}

function Release({ release, lang }: { release: ChangelogRelease; lang: string | undefined }) {
  const t = useText();
  return (
    <section aria-labelledby={release.id} className="flex flex-col gap-16">
      <div className="flex flex-col gap-4">
        <Heading id={release.id} title={release.version}>
          {release.latest && <Badge tone="ink">{t("changelog.latest")}</Badge>}
          {release.prerelease && <Badge tone="outline">{t("changelog.prerelease")}</Badge>}
          {release.yanked && <Badge tone="warning">{t("changelog.yanked")}</Badge>}
        </Heading>
        <div className="flex flex-wrap items-center gap-x-16 gap-y-4">
          {release.date && (
            <Text variant="body" tone="secondary" tabular>
              <time dateTime={release.date}>{releaseDate(release.date)}</time>
            </Text>
          )}
          {release.url && (
            <Link href={release.url} variant="quiet" external>
              {t("changelog.release")}
            </Link>
          )}
          {release.compare && release.previous && (
            <Link href={release.compare} variant="quiet" external>
              {t("changelog.compare").replace("{version}", release.previous)}
            </Link>
          )}
        </div>
      </div>
      <Suspense fallback={<div aria-hidden className="aspect-video w-full" />}>
        <ReleaseArtwork version={release.version} label={t("changelog.artwork").replace("{version}", release.version)} />
      </Suspense>
      <Entry entry={release} lang={lang} />
    </section>
  );
}

function ForwardToVersion({ elsewhere }: { elsewhere: Record<string, string> }) {
  const hash = useLocation({ select: (l) => l.hash });
  const navigate = useNavigate();
  const page = hash ? elsewhere[decodeURIComponent(hash)] : undefined;
  useEffect(() => {
    if (page) void navigate({ to: page, hash, replace: true });
  }, [page, hash, navigate]);
  return null;
}

function MoreReleases({ pages }: { pages: NonNullable<ChangelogContent["pages"]> }) {
  const t = useText();
  const path = useLocation({ select: (l) => l.pathname });
  return (
    <nav aria-label={t("changelog.pages")} className="flex flex-wrap items-center gap-x-16 gap-y-4">
      <Text variant="body" tone="secondary">
        {t("changelog.pages")}
      </Text>
      {pages
        .filter((p) => p.path !== path)
        .map((p) => (
          <Link key={p.path} href={p.path} variant="quiet">
            {p.series ? t("changelog.seriesTitle").replace("{series}", p.series) : t("changelog.current")}
          </Link>
        ))}
    </nav>
  );
}

export function Changelog({ changelog }: { changelog: ChangelogContent }) {
  const t = useText();
  const locale = useSiteLocale();
  const lang = locale === "en" ? undefined : "en";
  const { unreleased, releases, pages, elsewhere } = changelog;
  if (!unreleased && !releases.length)
    return (
      <Text as="p" variant="body-lg" weight="regular" tone="secondary" className="mt-40">
        {t("changelog.empty")}
      </Text>
    );
  return (
    <div className="mt-40 flex flex-col gap-40">
      {elsewhere && <ForwardToVersion elsewhere={elsewhere} />}
      {unreleased && (
        <section aria-labelledby={unreleased.id} className="flex flex-col gap-16">
          <div className="flex flex-col gap-4">
            <Heading id={unreleased.id} title={t("changelog.unreleased")} />
            <div className="flex flex-wrap items-center gap-x-16 gap-y-4">
              <Text variant="body" tone="secondary">
                {t("changelog.unreleasedNote")}
              </Text>
              {unreleased.compare && unreleased.since && (
                <Link href={unreleased.compare} variant="quiet" external>
                  {t("changelog.compare").replace("{version}", unreleased.since)}
                </Link>
              )}
            </div>
          </div>
          <Entry entry={unreleased} lang={lang} />
        </section>
      )}
      {releases.map((release, i) => (
        <Fragment key={release.id}>
          {(i > 0 || unreleased) && <Separator />}
          <Release release={release} lang={lang} />
        </Fragment>
      ))}
      {pages && <MoreReleases pages={pages} />}
    </div>
  );
}
