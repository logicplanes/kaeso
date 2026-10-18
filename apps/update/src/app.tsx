import { useCallback, useEffect, useState } from "react";
import {
  Badge,
  Button,
  Empty,
  Heading,
  Link,
  Logo,
  ReleaseArt,
  Separator,
  Skeleton,
  SkeletonLine,
  Text,
  ThemeProvider,
  TooltipProvider,
} from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { LocaleProvider, type UiMessages } from "@logicplanes/kaeso-ui/i18n";
import { Markdown } from "@logicplanes/kaeso-ui/markdown";
import { locale, t, type Locale } from "./i18n";
import { loadReleases, splitReleases, upgradeGuide, type Release } from "./releases";

export const COLLAPSE_AFTER = 360;

type Load = (signal: AbortSignal) => Promise<Release[]>;
type State = { status: "loading" } | { status: "error" } | { status: "ready"; releases: Release[] };

const defaultLoad: Load = (signal) => loadReleases(fetch, signal);

export function useReleases(load: Load): { state: State; retry: () => void } {
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<State>({ status: "loading" });
  useEffect(() => {
    const controller = new AbortController();
    load(controller.signal).then(
      (releases) => {
        if (!controller.signal.aborted) setState({ status: "ready", releases });
      },
      () => {
        if (!controller.signal.aborted) setState({ status: "error" });
      },
    );
    return () => controller.abort();
  }, [load, attempt]);
  const retry = useCallback(() => {
    setState({ status: "loading" });
    setAttempt((n) => n + 1);
  }, []);
  return { state, retry };
}

function published(release: Release): string | null {
  return release.publishedAt ? t("release.published", "Published {date}", { date: formatDate(release.publishedAt) }) : null;
}

function name(release: Release): string {
  return t("release.name", "Kaeso {version}", { version: release.version });
}

function Notes({ release, size, collapseAfter }: { release: Release; size: "body" | "small"; collapseAfter?: number }) {
  if (!release.notes)
    return (
      <Text as="p" variant={size} tone="secondary">
        {t("release.noNotes", "This release has no notes.")}
      </Text>
    );
  return <Markdown text={release.notes} size={size} headingLevel={size === "body" ? 2 : 3} collapseAfter={collapseAfter} />;
}

function Latest({ release }: { release: Release }) {
  const date = published(release);
  return (
    <section aria-label={name(release)} className="flex flex-col">
      <ReleaseArt
        shape="wide"
        seed={release.version}
        label={name(release)}
        eyebrow={release.prerelease ? t("hero.eyebrowPre", "Latest pre-release") : t("hero.eyebrow", "Latest release")}
        title={release.version}
      />
      <div className="mt-24 flex flex-wrap items-baseline justify-between gap-x-16 gap-y-4">
        <Heading level={2} size="title">
          {name(release)}
        </Heading>
        {date && <Text tone="secondary">{date}</Text>}
      </div>
      <div className="mt-16">
        <Notes release={release} size="body" />
      </div>
      <div className="mt-24 flex flex-wrap items-center gap-x-24 gap-y-8">
        <Link href={release.url} external>
          {t("release.onGitHub", "Release notes on GitHub")}
        </Link>
        <Link href={upgradeGuide(locale())} external>
          {t("hero.upgrade", "Upgrade guide")}
        </Link>
      </div>
    </section>
  );
}

function Earlier({ release, first }: { release: Release; first: boolean }) {
  const date = published(release);
  return (
    <li className="flex flex-col gap-24">
      {!first && <Separator />}
      <div className="flex flex-col gap-16 sm:flex-row sm:gap-24">
        <div className="w-160 shrink-0 sm:w-200">
          <ReleaseArt seed={release.version} label={name(release)} eyebrow="Kaeso" title={release.version} />
        </div>
        <div className="flex min-w-0 flex-1 flex-col gap-12">
          <div className="flex flex-col gap-4">
            <div className="flex flex-wrap items-center gap-8">
              <Heading level={3} size="title-sm">
                {name(release)}
              </Heading>
              {release.prerelease && <Badge tone="outline">{t("release.prerelease", "Pre-release")}</Badge>}
            </div>
            {date && (
              <Text variant="small" tone="secondary">
                {date}
              </Text>
            )}
          </div>
          <Notes release={release} size="small" collapseAfter={COLLAPSE_AFTER} />
          <div>
            <Link href={release.url} external variant="quiet">
              {t("release.onGitHub", "Release notes on GitHub")}
            </Link>
          </div>
        </div>
      </div>
    </li>
  );
}

function Others({ releases }: { releases: Release[] }) {
  if (!releases.length) return null;
  return (
    <section aria-labelledby="others" className="mt-64 flex flex-col gap-24">
      <Heading id="others" level={2} size="title-sm">
        {t("others.title", "Other releases")}
      </Heading>
      <ol className="flex flex-col gap-24">
        {releases.map((release, i) => (
          <Earlier key={release.version} release={release} first={i === 0} />
        ))}
      </ol>
    </section>
  );
}

function Loading() {
  return (
    <section aria-busy="true" aria-label={t("loading.label", "Loading releases")} className="flex flex-col">
      <Skeleton className="aspect-21/9 w-full" />
      <Skeleton shape="line" className="mt-24 w-200" />
      <SkeletonLine lines={4} className="mt-16" />
    </section>
  );
}

function Releases({ releases }: { releases: Release[] }) {
  const { latest, others } = splitReleases(releases);
  if (!latest)
    return <Empty title={t("empty.title", "No releases yet")} description={t("empty.description", "Published releases appear here with their notes.")} />;
  return (
    <>
      <Latest release={latest} />
      <Others releases={others} />
    </>
  );
}

export function App({ load = defaultLoad }: { load?: Load }) {
  const { state, retry } = useReleases(load);
  return (
    <div className="flex min-h-dvh flex-col">
      <header className="ml-28 mt-28">
        <Logo.Wordmark size="nav" />
      </header>
      <main className="mx-28 mt-48 mb-80 flex flex-col">
        <div className="mx-auto flex w-full max-w-720 flex-col">
          <div className="mb-32 flex flex-col gap-8">
            <Heading level={1} size="title-lg">
              {t("page.heading", "Releases")}
            </Heading>
            <Text variant="body-lg" tone="secondary">
              {t("page.description", "Every Kaeso release, its notes and how to upgrade.")}
            </Text>
          </div>
          {state.status === "loading" && <Loading />}
          {state.status === "error" && (
            <Empty
              title={t("error.title", "Releases didn't load")}
              description={t("error.description", "Check your connection, then try again.")}
              actions={<Button onClick={retry}>{t("error.retry", "Try again")}</Button>}
            />
          )}
          {state.status === "ready" && <Releases releases={state.releases} />}
        </div>
      </main>
    </div>
  );
}

export function Root({ locale: tag, ui, load }: { locale: Locale; ui: UiMessages; load?: Load }) {
  return (
    <ThemeProvider>
      <LocaleProvider locale={tag} messages={ui}>
        <TooltipProvider>{load ? <App load={load} /> : <App />}</TooltipProvider>
      </LocaleProvider>
    </ThemeProvider>
  );
}
