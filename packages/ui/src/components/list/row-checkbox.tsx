import { useEffect, useState } from "react";
import { cn } from "../../utils/cn.js";
import { lazyModule, useLazyModule } from "../../utils/lazy-module.js";

const checkbox = lazyModule(() => import("../checkbox/checkbox.js"));

export function RowCheckbox({ checked, onToggle, "aria-label": ariaLabel }: { checked: boolean; onToggle: () => void; "aria-label": string }) {
  const mod = useLazyModule(checkbox, true);
  const [over, setOver] = useState(false);
  const [pressed, setPressed] = useState(false);
  useEffect(() => {
    if (!pressed) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const release = () => {
      timer = setTimeout(() => setPressed(false));
    };
    window.addEventListener("pointerup", release, true);
    window.addEventListener("pointercancel", release, true);
    return () => {
      clearTimeout(timer);
      window.removeEventListener("pointerup", release, true);
      window.removeEventListener("pointercancel", release, true);
    };
  }, [pressed]);
  if (mod && !over && !pressed) return <mod.CheckboxBox checked={checked} onCheckedChange={onToggle} aria-label={ariaLabel} />;
  return (
    <span
      role="checkbox"
      aria-checked={checked}
      aria-label={ariaLabel}
      onPointerEnter={() => setOver(true)}
      onPointerLeave={() => setOver(false)}
      onPointerDown={() => setPressed(true)}
      onClick={() => {
        onToggle();
        setOver(false);
        setPressed(false);
      }}
      className="relative -m-3 inline-flex size-24 shrink-0 cursor-pointer items-center justify-center rounded-checkbox"
    >
      <span aria-hidden className={cn("flex size-18 items-center justify-center rounded-checkbox", checked ? "bg-ink" : "bg-raised shadow-selector")} />
    </span>
  );
}
