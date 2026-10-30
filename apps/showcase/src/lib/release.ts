import repository from "../../../../package.json";
import { GITHUB } from "./nav";

export const RELEASE_TAG = `v${repository.version}`;

export function sourceUrl(path: string): string {
  return `${GITHUB}/tree/${RELEASE_TAG}/${path.replace(/^\/+/, "")}`;
}
