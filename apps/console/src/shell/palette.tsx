import { Suspense, lazy, useCallback, useEffect, useState } from "react";
import { onPaletteOpen, setPaletteShown } from "./palette-store";

const PaletteUI = lazy(() => import("./palette-ui"));

const typesIntoPalette = (target: EventTarget | null) => target instanceof Element && target.closest("[data-palette-input]") !== null;

export function CommandPaletteHost() {
  const [open, setOpen] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [query, setQuery] = useState("");
  useEffect(() => setPaletteShown(open), [open]);
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey || typesIntoPalette(e.target)) return;
      if (e.key.length === 1) {
        e.preventDefault();
        setQuery((q) => q + e.key);
      } else if (e.key === "Backspace") {
        e.preventDefault();
        setQuery((q) => q.slice(0, -1));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);
  const show = useCallback((o: boolean) => {
    if (o) {
      setLoaded(true);
      setQuery("");
    }
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
      <PaletteUI open={open} onOpenChange={show} query={query} onQueryChange={setQuery} />
    </Suspense>
  );
}
