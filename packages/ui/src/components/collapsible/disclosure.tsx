import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";

export const DISCLOSURE_TRIGGER =
  "group/dt relative flex w-full cursor-pointer items-center gap-8 rounded-pill px-12 text-left transition-control focus-ring-inset";

export function DisclosureChevron() {
  return (
    <span className="flex shrink-0 text-fg-tertiary transition-control group-hover/dt:text-fg-secondary">
      <Icon icon={icons.chevronRight} size="sm" className="transition-transform duration-200 ease-overlay group-data-panel-open/dt:rotate-90" />
    </span>
  );
}
