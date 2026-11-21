import repository from "../../../../package.json";
import source from "~/generated/source.json";
import { GITHUB } from "./nav";

/**
 * The commit the site is built from, so View source opens a file that exists: a pre-release build shows components that
 * no release tag contains yet. The release tag only where the build has no checkout to read (scripts/source-ref.mjs).
 */
const commit: string | null = source.commit;
export const SOURCE_REF = commit ?? `v${repository.version}`;

export function sourceUrl(path: string): string {
  return `${GITHUB}/tree/${SOURCE_REF}/${path.replace(/^\/+/, "")}`;
}
