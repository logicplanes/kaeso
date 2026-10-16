import { Fragment, isValidElement, type ReactElement, type ReactNode } from "react";
import { t, type MessageValues } from "./index";

const MARK = "\u0000";

export function tRich(key: string, english: string, values: Record<string, ReactElement | string | number>): ReactNode {
  const nodes: ReactNode[] = [];
  const plain: MessageValues = {};
  for (const [name, value] of Object.entries(values)) {
    if (isValidElement(value)) {
      plain[name] = `${MARK}${nodes.length}${MARK}`;
      nodes.push(value);
    } else plain[name] = value;
  }
  const parts = t(key, english, plain).split(MARK);
  return parts.map((part, i) => <Fragment key={i}>{i % 2 ? nodes[Number(part)] : part}</Fragment>);
}
