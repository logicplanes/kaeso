import type { ComponentProps } from "react";
import { vi } from "vitest";

vi.mock("@logicplanes/kaeso-ui", async (importOriginal) => {
  const ui = await importOriginal<typeof import("@logicplanes/kaeso-ui")>();
  const { useTooltipsAtOnce } = await import("./tooltip-delay");
  function TooltipProvider(props: ComponentProps<typeof ui.TooltipProvider>) {
    const atOnce = useTooltipsAtOnce();
    return <ui.TooltipProvider {...props} {...(atOnce ? { delay: 0 } : {})} />;
  }
  return { ...ui, TooltipProvider };
});
