import { Fragment, type ReactNode } from "react";
import { Code } from "@logicplanes/kaeso-ui";

export function CodeText({ text }: { text: string }): ReactNode {
  const parts = text.split(/`([^`]+)`/);
  if (parts.length === 1) return text;
  return parts.map((part, i) =>
    i % 2 ? (
      <Code key={i} flow="cell">
        {part}
      </Code>
    ) : (
      <Fragment key={i}>{part}</Fragment>
    ),
  );
}
