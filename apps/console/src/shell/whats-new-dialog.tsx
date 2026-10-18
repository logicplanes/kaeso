import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Button, Dialog, DialogScope, Link, ReleaseArt, Text, icons, isReleaseArtSpec } from "@logicplanes/kaeso-ui";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { Markdown } from "@logicplanes/kaeso-ui/markdown";
import { api } from "~/api/client";
import type { WhatsNewRelease } from "~/api/generated/types";
import { whatsNewQuery } from "~/api/queries/whats-new";
import { t } from "~/i18n";

export default function WhatsNew({ releases }: { releases: WhatsNewRelease[] }) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(true);
  const close = () => {
    setOpen(false);
    void api.POST("/platform/v1/me/whats-new/seen").then(() => queryClient.invalidateQueries({ queryKey: whatsNewQuery.queryKey }));
  };
  return (
    <DialogScope open={open}>
      <WhatsNewDialog releases={releases} open={open} onOpenChange={close} />
    </DialogScope>
  );
}

function WhatsNewDialog({ releases, open, onOpenChange }: { releases: WhatsNewRelease[]; open: boolean; onOpenChange: () => void }) {
  const [index, setIndex] = useState(0);
  const [details, setDetails] = useState(false);
  const release = releases[index] as WhatsNewRelease;
  const last = index >= releases.length - 1;
  const hasNotes = release.notes.trim().length > 0;
  const go = (to: number) => {
    setIndex(to);
    setDetails(false);
  };
  const when = release.published_at
    ? t("shell.whatsNew.published", "Published {date}", { date: formatDate(release.published_at) })
    : t("shell.whatsNew.installed", "Now running on this install");
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      size="md"
      title={t("shell.whatsNew.title", "What's new in Kaeso")}
      description={
        releases.length > 1 ? t("shell.whatsNew.page", "{when} · {current} of {count} releases", { when, current: index + 1, count: releases.length }) : when
      }
      footer={
        <>
          <div className="mr-auto">
            {hasNotes ? (
              <Button trailingIcon={details ? icons.chevronUp : icons.chevronDown} onClick={() => setDetails((d) => !d)}>
                {details ? t("shell.whatsNew.hide", "Hide improvements") : t("shell.whatsNew.show", "Show improvements")}
              </Button>
            ) : (
              <Button href={release.url} target="_blank" rel="noopener noreferrer" trailingIcon={icons.externalLink}>
                {t("shell.whatsNew.onGitHub", "Release notes on GitHub")}
              </Button>
            )}
          </div>
          {index > 0 && <Button onClick={() => go(index - 1)}>{t("shell.whatsNew.back", "Back")}</Button>}
          {last ? (
            <Button variant="primary" onClick={onOpenChange}>
              {t("shell.whatsNew.continue", "Continue")}
            </Button>
          ) : (
            <Button variant="primary" onClick={() => go(index + 1)}>
              {t("shell.whatsNew.next", "Next")}
            </Button>
          )}
        </>
      }
    >
      <div className="flex flex-col gap-16">
        <ReleaseArt
          seed={release.version}
          art={isReleaseArtSpec(release.art) ? release.art : undefined}
          label={t("shell.whatsNew.artLabel", "Kaeso {version}", { version: release.version })}
          eyebrow={release.prerelease ? t("shell.whatsNew.preEyebrow", "Kaeso pre-release") : t("shell.whatsNew.eyebrow", "Kaeso")}
          title={release.version}
        />
        {details && (
          <section aria-label={t("shell.whatsNew.notesOf", "Improvements in {version}", { version: release.version })} className="flex flex-col gap-8">
            <Markdown text={release.notes} headingLevel={3} size="small" />
            <Link href={release.url} external variant="quiet">
              {t("shell.whatsNew.onGitHub", "Release notes on GitHub")}
            </Link>
          </section>
        )}
        {!details && !hasNotes && (
          <Text variant="small" tone="secondary">
            {t("shell.whatsNew.noNotes", "The notes of this release aren't on this install yet.")}
          </Text>
        )}
      </div>
    </Dialog>
  );
}
