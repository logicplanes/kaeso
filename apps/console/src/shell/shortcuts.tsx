import { Suspense, lazy, useEffect, useRef, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useCreateAction } from "./create-action";
import { takeEarlyKeys } from "./early-keys";
import { useGoKeys } from "./go-keys";
import { isPaletteOpen } from "./palette-store";
import { onShortcutSheet, openShortcutSheet } from "./shortcuts-store";

function typing(target: EventTarget | null): boolean {
  return target instanceof Element && !!target.closest("input, textarea, select, [contenteditable=true], [role=combobox]");
}

function layerOpen(): boolean {
  return !!document.querySelector("[role=dialog], [role=menu], [role=listbox]");
}

export function ShortcutsHost() {
  const navigate = useNavigate();
  const create = useCreateAction();
  const goKeys = useGoKeys();
  const pendingG = useRef<number>(0);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey || typing(e.target) || layerOpen() || isPaletteOpen()) return;
      if (Date.now() - pendingG.current < 1200) {
        pendingG.current = 0;
        const go = goKeys.find((k) => k.key === e.key);
        if (go) {
          e.preventDefault();
          void navigate({ to: go.to });
        }
        return;
      }
      switch (e.key) {
        case "g":
          pendingG.current = Date.now();
          return;
        case "c":
          if (create && !create.disabledReason) {
            e.preventDefault();
            create.run();
          }
          return;
        case "/": {
          const search = document.querySelector<HTMLInputElement>("main input[type=search], [data-page-search] input");
          if (search) {
            e.preventDefault();
            search.focus();
          }
          return;
        }
        case "?":
          e.preventDefault();
          openShortcutSheet();
          return;
      }
    };
    window.addEventListener("keydown", onKey);
    for (const e of takeEarlyKeys()) onKey(e);
    return () => window.removeEventListener("keydown", onKey);
  }, [create, goKeys, navigate]);

  return <ShortcutSheetHost />;
}

const ShortcutSheet = lazy(() => import("./shortcut-sheet"));

function ShortcutSheetHost() {
  const [open, setOpen] = useState(false);
  const [loaded, setLoaded] = useState(false);
  useEffect(
    () =>
      onShortcutSheet(() => {
        setLoaded(true);
        setOpen(true);
      }),
    [],
  );
  if (!loaded) return null;
  return (
    <Suspense fallback={null}>
      <ShortcutSheet open={open} onOpenChange={setOpen} />
    </Suspense>
  );
}
