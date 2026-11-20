import { Segmented, Select } from "@logicplanes/kaeso-ui";
import { usePhone } from "~/shell/use-phone";

/** How a breakdown groups its rows: segments where they fit, and a menu on a phone, where most of the segments would sit out of sight. */
export function GroupBy<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}) {
  const phone = usePhone();
  const pick = (v: string) => {
    const next = options.find((o) => o.value === v);
    if (next) onChange(next.value);
  };
  return phone ? (
    <Select aria-label={label} size="sm" value={value} onValueChange={pick} options={options} />
  ) : (
    <Segmented aria-label={label} size="sm" value={value} onValueChange={pick} options={options} />
  );
}
