import { useEffect, useRef, type ReactNode } from "react";

export function Labelled({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-8">
      <span className="text-caption font-medium text-fg-tertiary">{title}</span>
      {children}
    </div>
  );
}

export function AsTable({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const buttons = ref.current?.querySelectorAll<HTMLButtonElement>("figure button");
    buttons?.[buttons.length - 1]?.click();
  }, []);
  return <div ref={ref}>{children}</div>;
}
