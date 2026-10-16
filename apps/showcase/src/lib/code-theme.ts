import type { ShikiConfig } from "astro";
import { kaesoShikiTheme } from "../../../../packages/ui/src/components/code-block/kaeso-theme";

export const codeTheme: NonNullable<ShikiConfig["theme"]> = kaesoShikiTheme;
