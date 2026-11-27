import { Announcement, Button, Dashboard, Skeleton } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import { dismissedRelease } from "./release-dismissed";
import { showWhatsNew } from "~/shell/whats-new-request";

export function ReadWhatsNew() {
  return (
    <Button size="sm" onClick={() => showWhatsNew()}>
      {t("kaeso.home.readWhatsNew", "Read what's new")}
    </Button>
  );
}

// What's new always has the running release, so the banner keeps its place while it loads; its summary always takes
// two lines' room. Someone who closed a release's banner most likely still has it closed, so it holds no place then.
export function ReleaseBannerPending() {
  if (dismissedRelease() !== null) return null;
  return (
    <Dashboard aria-label={t("kaeso.home.news", "News")} busy>
      <Announcement
        className="col-span-full"
        eyebrow={<Skeleton shape="word" className="w-80" />}
        title={<Skeleton shape="word" className="w-160" />}
        description={
          <>
            <Skeleton shape="word" className="w-full" />
            <Skeleton shape="word" className="w-3/5" />
          </>
        }
        actions={<ReadWhatsNew />}
      />
    </Dashboard>
  );
}
