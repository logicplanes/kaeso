export function OptionCheckbox() {
  return (
    <span className="inline-flex size-18 shrink-0 items-center justify-center rounded-checkbox bg-raised shadow-selector group-data-selected/opt:bg-ink group-data-selected/opt:shadow-none">
      <svg viewBox="0 0 12 12" className="hidden size-12 text-on-ink group-data-selected/opt:block" fill="none" aria-hidden>
        <path d="M2.5 6.25 4.9 8.5 9.5 3.75" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </span>
  );
}
