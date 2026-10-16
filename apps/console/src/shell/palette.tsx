import { Suspense, lazy, useCallback, useEffect, useState } from "react";
import { onPaletteOpen, setPaletteShown } from "./palette-store";

const PaletteUI = lazy(() => import("./palette-ui"));

const typesIntoField = (target: EventTarget | null) => target instanceof HTMLElement && target.closest("input, textarea, [contenteditable]") !== null;

export function CommandPaletteHost() {
  const [open, setOpen] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [ready, setReady] = useState(false);
  const [typed, setTyped] = useState("");
  useEffect(() => setPaletteShown(open), [open]);
  useEffect(() => {
    if (!open || ready) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey || typesIntoField(e.target)) return;
      if (e.key.length === 1) {
        e.preventDefault();
        setTyped((t) => t + e.key);
      } else if (e.key === "Backspace") {
        e.preventDefault();
        setTyped((t) => t.slice(0, -1));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, ready]);
  const show = useCallback((o: boolean) => {
    if (o) setLoaded(true);
    else setTyped("");
    setOpen(o);
  }, []);
  useEffect(() => onPaletteOpen(show), [show]);
  useEffect(() => {
    const t = setTimeout(() => void import("./palette-ui"), 1500);
    return () => clearTimeout(t);
  }, []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        show(!open);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, show]);
  if (!loaded) return null;
  return (
    <Suspense fallback={null}>
      <PaletteUI open={open} onOpenChange={show} initialQuery={typed} onReady={() => setReady(true)} />
    </Suspense>
  );
}
