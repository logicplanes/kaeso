import type { ReactNode } from "react";
import { StateCell, StateMatrix, StateRow } from "./states";

export interface GridState {
  label: string;
  id: string;
  props: Record<string, unknown>;
}

export const FIELD_STATES: GridState[] = [
  { label: "Rest", id: "rest", props: {} },
  { label: "Hover", id: "hover", props: { "data-force": "hover" } },
  { label: "Focus", id: "focus", props: { "data-force": "focus" } },
  { label: "Invalid", id: "invalid", props: { invalid: true } },
  { label: "Disabled", id: "disabled", props: { disabled: true } },
  { label: "Read-only", id: "read-only", props: { readOnly: true } },
];

export function StateGrid<K extends { label: string; id: string }>({
  title,
  kinds,
  states,
  render,
}: {
  title: string;
  kinds: K[];
  states: GridState[];
  render: (kind: K, props: Record<string, unknown>, state: GridState) => ReactNode;
}) {
  return (
    <StateMatrix title={title} columns={kinds.map((k) => k.label)}>
      {states.map((s) => (
        <StateRow key={s.id} label={s.label}>
          {kinds.map((k) => (
            <StateCell key={k.id} id={`${k.id}-${s.id}`}>
              {render(k, s.props, s)}
            </StateCell>
          ))}
        </StateRow>
      ))}
    </StateMatrix>
  );
}
