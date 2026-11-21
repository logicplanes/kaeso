import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

const OBJECT_ID = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/;

function read(file) {
  try {
    return readFileSync(file, "utf8").trim();
  } catch {
    return null;
  }
}

/**
 * The commit a checkout at `dir` (or a folder above it) has out, read from its `.git` without running git: the site is
 * also built where git isn't installed, such as a container's build stage with a clone of the tag copied in. Covers a
 * detached HEAD, a branch in loose or packed refs, and a linked worktree, whose `.git` is a file naming its folder.
 * Null outside a checkout.
 */
export function commitOf(dir) {
  let at = resolve(dir);
  while (!existsSync(join(at, ".git"))) {
    const up = dirname(at);
    if (up === at) return null;
    at = up;
  }
  let gitDir = join(at, ".git");
  if (statSync(gitDir).isFile()) {
    const pointer = /^gitdir:\s*(.+)$/m.exec(read(gitDir) ?? "");
    if (!pointer) return null;
    gitDir = resolve(at, pointer[1].trim());
  }
  const common = read(join(gitDir, "commondir"));
  const commonDir = common ? resolve(gitDir, common) : gitDir;
  const head = read(join(gitDir, "HEAD")) ?? "";
  if (OBJECT_ID.test(head)) return head;
  const ref = /^ref:\s*(\S+)$/.exec(head)?.[1];
  if (!ref) return null;
  for (const file of [join(gitDir, ref), join(commonDir, ref)]) {
    const id = read(file);
    if (id && OBJECT_ID.test(id)) return id;
  }
  for (const line of (read(join(commonDir, "packed-refs")) ?? "").split("\n")) {
    const [id, name] = line.split(" ");
    if (name === ref && OBJECT_ID.test(id ?? "")) return id;
  }
  return null;
}
