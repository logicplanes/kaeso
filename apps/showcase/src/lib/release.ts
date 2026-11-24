import repository from "../../../../package.json";
import source from "~/generated/source.json";
import { GITHUB } from "./nav";

/**
 * The commit the site is built from, so View source opens a file that exists: a pre-release build shows components that
 * no release tag contains yet. The release tag only where the build has no checkout to read (scripts/source-ref.mjs).
 */
const commit: string | null = source.commit;
export const SOURCE_REF = commit ?? `v${repository.version}`;

/** A file opens as a file (blob), a folder as a folder (tree): GitHub answers 404 to the other. */
export function sourceUrl(path: string): string {
  const clean = path.replace(/^\/+/, "");
  return `${GITHUB}/${/\.[a-z]+$/i.test(clean) ? "blob" : "tree"}/${SOURCE_REF}/${clean}`;
}
