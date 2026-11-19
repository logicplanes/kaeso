import { Tooltip, VisuallyHidden } from "@logicplanes/kaeso-ui";
import type { LimitFigureText } from "~/format/limit-use";

export function LimitFigure({ figure }: { figure: LimitFigureText }) {
  if (figure.short === figure.exact) return <>{figure.short}</>;
  return (
    <Tooltip content={figure.exact}>
      <span>
        <span aria-hidden>{figure.short}</span>
        <VisuallyHidden>{figure.exact}</VisuallyHidden>
      </span>
    </Tooltip>
  );
}
