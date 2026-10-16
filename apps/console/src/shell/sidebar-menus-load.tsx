import { useEffect, useRef, useState, type ReactNode, type SyntheticEvent } from "react";

type Menus = typeof import("./sidebar-menus");
let loaded: Menus | undefined;
let loading: Promise<Menus | undefined> | undefined;

const loadSidebarMenus = () =>
  (loading ??= import("./sidebar-menus").then(
    (m) => (loaded = m),
    () => (loading = undefined),
  ));

void loadSidebarMenus();

export function useSidebarMenus(): Menus | undefined {
  const [menus, setMenus] = useState(() => loaded);
  useEffect(() => {
    if (!menus) void loadSidebarMenus().then((m) => m && setMenus(m));
  }, [menus]);
  return menus;
}

const OPENING_KEYS = new Set(["Enter", " ", "ArrowDown", "ArrowUp"]);

export function OpensWhenReady({ ready, children }: { ready: boolean; children: ReactNode }) {
  const box = useRef<HTMLDivElement>(null);
  const held = useRef(false);
  useEffect(() => {
    if (!ready || !held.current) return;
    held.current = false;
    box.current?.querySelector("button")?.click();
  }, [ready]);
  const hold = (e: SyntheticEvent) => {
    if (ready) return;
    e.preventDefault();
    e.stopPropagation();
    held.current = true;
  };
  const holdKey = (e: SyntheticEvent & { key: string }) => {
    if (OPENING_KEYS.has(e.key)) hold(e);
  };
  return (
    <div ref={box} className="contents" onPointerDownCapture={hold} onMouseDownCapture={hold} onClickCapture={hold} onKeyDownCapture={holdKey}>
      {children}
    </div>
  );
}
