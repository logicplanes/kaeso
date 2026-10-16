import { useEffect } from "react";
import { useRouter } from "@tanstack/react-router";
import { ConnectionToast } from "./connection-banner";
import { ShortcutsHost } from "./shortcuts";
import { LocaleSync } from "~/i18n/sync";

function BackScroll() {
  const router = useRouter();
  useEffect(() => {
    let traversal = false;
    router.update({ ...router.options, scrollRestoration: () => traversal, scrollToTopSelectors: ["main > div"] });
    return router.history.subscribe(({ action }) => {
      traversal = action.type !== "PUSH" && action.type !== "REPLACE";
    });
  }, [router]);
  return null;
}

export default function ShellListeners() {
  return (
    <>
      <ConnectionToast />
      <ShortcutsHost />
      <BackScroll />
      <LocaleSync />
    </>
  );
}
