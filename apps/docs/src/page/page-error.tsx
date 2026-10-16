import { useEffect } from "react";
import { useRouter } from "@tanstack/react-router";
import { AppFrame, Button, Empty, VisuallyHidden } from "@logicplanes/kaeso-ui";
import { useText } from "~/i18n/text";
import { toast } from "~/shell/toast";
import { usePageTitle } from "~/shell/title";

function useRetry(reset: () => void): () => void {
  const router = useRouter();
  return () => {
    reset();
    void router.invalidate();
  };
}

function PageErrorContent({ reset }: { reset: () => void }) {
  const t = useText();
  const retry = useRetry(reset);
  usePageTitle(t("pageError.title"));
  useEffect(() => toast.error(t("pageError.toast"), { id: "page-load" }), [t]);
  return (
    <>
      <VisuallyHidden>
        <h1>{t("pageError.title")}</h1>
      </VisuallyHidden>
      <Empty
        size="lg"
        title={t("pageError.heading")}
        description={t("pageError.description")}
        actions={<Button onClick={retry}>{t("pageError.retry")}</Button>}
      />
    </>
  );
}

export function PageError({ reset }: { reset: () => void }) {
  return (
    <AppFrame.Body className="relative">
      <AppFrame.MenuButton className="absolute top-10 left-12 z-10" />
      <PageErrorContent reset={reset} />
    </AppFrame.Body>
  );
}

export function BootError({ reset }: { reset: () => void }) {
  return (
    <main className="flex h-dvh items-center justify-center">
      <PageErrorContent reset={reset} />
    </main>
  );
}
