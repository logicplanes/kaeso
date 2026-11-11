import { formatTimeZone } from "../format/index.js";
import { useHydrating } from "./use-hydrating.js";

// The server can't know the viewer's zone, so the server render and the hydrating render agree on UTC unless the app set one zone for both.
export const useUtcWhileHydrating = (): boolean => useHydrating() && !formatTimeZone();
