import { useCallback, useEffect, useRef, useState } from "react";

export function useScrolledUnder<T extends HTMLElement>() {
  const ref = useRef<T | null>(null);
  const [under, setUnder] = useState(false);
  const [scrolled, setScrolled] = useState(false);
  const check = useCallback(() => {
    const el = ref.current;
    if (!el || el.closest("[data-resizing]")) return;
    setUnder(el.scrollHeight - el.scrollTop - el.clientHeight > 1);
    setScrolled(el.scrollTop > 0);
  }, []);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    check();
    el.addEventListener("scroll", check, { passive: true });
    window.addEventListener("resize", check);
    return () => {
      el.removeEventListener("scroll", check);
      window.removeEventListener("resize", check);
    };
  }, [check]);
  return { ref, under, scrolled, check };
}
