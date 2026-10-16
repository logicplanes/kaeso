import { down, up } from "../scripts/stack.mjs";
import { STACK } from "./stack";

export default function globalSetup() {
  if (process.env.KAESO_E2E_STACK === "running") return;
  down(STACK);
  up(STACK);
  return () => down(STACK);
}
