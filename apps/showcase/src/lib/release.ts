import repository from "../../../../package.json";
import { GITHUB } from "./nav";

export const RELEASE_TAG = `v${repository.version}`;

export function sourceUrl(path: string): string {
  return `${GITHUB}/tree/${RELEASE_TAG}/${path.replace(/^\/+/, "")}`;
}

const literal = (s: string) => s.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
const NAME = GITHUB.replace(/^https:\/\/github\.com\//, "");
const ON_DEV = [
  new RegExp(`(${literal(GITHUB)}/(?:tree|blob|raw))/dev(?=[/"#?]|$)`, "gm"),
  new RegExp(`(https://raw\\.githubusercontent\\.com/${literal(NAME)})/dev(?=/)`, "g"),
];

export function pinToRelease(text: string): string {
  return ON_DEV.reduce((out, pattern) => out.replace(pattern, `$1/${RELEASE_TAG}`), text);
}
