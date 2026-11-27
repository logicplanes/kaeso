import { readStored, writeStored } from "~/storage";

const KEY = "kaeso:release-dismissed";

export const dismissedRelease = () => readStored(KEY);

export const dismissRelease = (version: string) => writeStored(KEY, version);
