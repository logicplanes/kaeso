import { useEffect } from "react";
import { AppFrame, Link, Separator, TableOfContents, Text } from "@logicplanes/kaeso-ui";
import { useText } from "~/i18n/text";
import type { PageContent, PageMeta } from "~/markdown/ast";
import { Markdown } from "~/markdown/render";
import { usePageTitle } from "~/shell/title";
import { Changelog, ChangelogNote } from "./changelog";
import type { LoadedPage } from "./load";

export function DocPage({ meta, content, editBase }: LoadedPage) {
  usePageTitle(meta.title);
  useEffect(() => {
    const id = decodeURIComponent(location.hash.slice(1));
    if (id) document.getElementById(id)?.scrollIntoView();
  }, [meta.path]);
  return (
    <AppFrame.Body className="relative">
      <AppFrame.MenuButton className="absolute top-10 left-12 z-10" />
      <div className="mx-24 mt-24 mb-96 flex justify-center gap-64 md:mx-40 lg:mt-64">
        <article className="max-w-640 min-w-0 flex-1">
          <header className="flex flex-col gap-8">
            <Text as="h1" variant="title-lg" weight="semibold" className="text-balance wrap-break-word">
              {meta.title}
            </Text>
            <Text as="p" variant="body-lg" weight="regular" tone="secondary" className="text-pretty">
              {meta.description}
            </Text>
            {content.changelog && <ChangelogNote />}
          </header>
          {content.changelog ? (
            <Changelog changelog={content.changelog} />
          ) : (
            content.blocks.length > 0 && <Markdown blocks={content.blocks} className="mt-40" />
          )}
          <PageFooter meta={meta} editBase={editBase} content={content} />
        </article>
        <div className="hidden w-200 shrink-0 xl:block">{content.toc.length > 1 && <TableOfContents items={content.toc} className="sticky top-64" />}</div>
      </div>
    </AppFrame.Body>
  );
}

function PageFooter({ meta, editBase, content }: { meta: PageMeta; editBase: string; content: PageContent }) {
  const t = useText();
  return (
    <footer className="mt-64 flex flex-col gap-24">
      {(meta.prev || meta.next) && (
        <nav aria-label={t("page.steps")} className="grid grid-cols-2 gap-16">
          <div className="flex min-w-0">
            {meta.prev && <Step to={meta.prev} label={t("page.previous")} name={t("page.previousStep").replace("{title}", meta.prev.title)} />}
          </div>
          <div className="flex min-w-0 justify-end">
            {meta.next && <Step to={meta.next} label={t("page.next")} name={t("page.nextStep").replace("{title}", meta.next.title)} end />}
          </div>
        </nav>
      )}
      <Separator />
      <div className="flex">
        {content.changelog ? (
          content.changelog.repository && (
            <Link href={`${content.changelog.repository}/releases`} variant="quiet" external>
              {t("changelog.releases")}
            </Link>
          )
        ) : (
          <Link href={`${editBase}${meta.file}`} variant="quiet" external>
            {t("page.edit")}
          </Link>
        )}
      </div>
    </footer>
  );
}

function Step({ to, label, name, end = false }: { to: { path: string; title: string }; label: string; name: string; end?: boolean }) {
  return (
    <Link
      href={to.path}
      variant="standalone"
      aria-label={name}
      className={`flex max-w-full min-w-0 flex-col gap-2 ${end ? "items-end text-right" : "items-start"}`}
    >
      <Text variant="small" tone="secondary">
        {label}
      </Text>
      {/* The flex gap is no space in the link's text, which label-in-name (WCAG 2.5.3) would read as "NextChangelog". */}{" "}
      <span className="max-w-full wrap-break-word">{to.title}</span>
    </Link>
  );
}
