import { Calendar, DatePicker, DateRangePicker, TimeInput } from "@logicplanes/kaeso-ui";
import { Matrix, StateSection } from "~/lib/states";

export const meta = {
  title: "DatePicker",
  description: "A pill showing the date; a calendar with circles, today ringed, and a typed ISO field.",
  group: "Components",
  entry: "date-picker",
  import: "DatePicker, DateRangePicker, Calendar, TimeInput",
  keyboard: [
    ["Arrows", "Move between days"],
    ["Page Up Page Down", "Previous and next month"],
    ["Enter", "Chooses the day"],
    ["Up Down", "Move between the range's presets"],
  ],
  a11y: [
    "The calendar is a grid with day names; typed ISO fields (YYYY-MM-DD) are an alternative to pointing.",
    "The calendar, the trigger and the typed fields name the same day, in the viewer's calendar.",
    "A field's label names the trigger and the date or range it shows is its description; without a label, the trigger is named by what it shows and its aria-label is its description.",
  ],
};
export const examples = [
  { id: "single", title: "Date", fn: "Single" },
  {
    id: "range",
    title: "Range with presets",
    fn: "Range",
    description: "Presets first: Last 24 hours, 7 days, 30 days, This month, Last month, Custom. A preset applies at once; chosen days with Apply.",
  },
  { id: "time", title: "Time", fn: "Time" },
];
export function Single() {
  return <DatePicker label="Expires" defaultValue={new Date(2027, 0, 1)} />;
}
export function Range() {
  return <DateRangePicker aria-label="Period" defaultValue={{ from: new Date(2026, 9, 1), to: new Date(2026, 9, 12) }} />;
}
export function Time() {
  return <TimeInput label="Daily report at" defaultValue="09:30" utc />;
}
const day = new Date(2026, 9, 12);
const range = { from: new Date(2026, 9, 1), to: new Date(2026, 9, 12) };

function Trigger({ row, props }: { row: string; props: Record<string, unknown> }) {
  if (row === "range") return <DateRangePicker aria-label="Period" size="sm" defaultValue={range} {...props} />;
  return <DatePicker aria-label="Expires" size="sm" {...(row === "date" ? { defaultValue: day } : {})} {...props} />;
}

export function States() {
  return (
    <>
      <Matrix
        title="Trigger"
        rows={[{ label: "date" }, { label: "range" }, { label: "empty" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Pressed", props: { "data-force": "pressed" } },
        ]}
        render={(r, p) => <Trigger row={r.label} props={p} />}
      />
      <Matrix
        title="Focus, open and off"
        rows={[{ label: "date" }, { label: "range" }, { label: "empty" }]}
        columns={[
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Open", props: { "data-force": "open" } },
          { label: "Disabled", props: { disabled: true } },
        ]}
        render={(r, p) => <Trigger row={r.label} props={p} />}
      />
      <StateSection title="Sizes and errors">
        <div className="flex flex-col items-start gap-12">
          <DatePicker aria-label="Small" size="sm" defaultValue={day} />
          <DatePicker aria-label="Medium" defaultValue={day} />
          <DatePicker aria-label="Large" size="lg" defaultValue={day} />
          <DatePicker label="Expires" error="Choose a date after today." defaultValue={new Date(2026, 9, 1)} />
        </div>
      </StateSection>
      <StateSection title="Calendar">
        <div className="flex flex-wrap gap-32">
          <Calendar month={new Date(2026, 9, 1)} selected={new Date(2026, 9, 14)} disabled={(d) => d > new Date(2026, 9, 24)} />
          <Calendar mode="range" month={new Date(2026, 9, 1)} selected={{ from: new Date(2026, 9, 4), to: new Date(2026, 9, 12) }} />
        </div>
      </StateSection>
      <StateSection title="Time">
        <div className="flex flex-col items-start gap-12">
          <TimeInput aria-label="Time" defaultValue="09:30" />
          <TimeInput aria-label="Focus" defaultValue="23:05" data-force="focus" utc />
          <TimeInput aria-label="Disabled" defaultValue="09:30" disabled />
          <TimeInput label="Send at" defaultValue="07:45" error="Choose a time inside the maintenance window." />
        </div>
      </StateSection>
    </>
  );
}
