import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { commitOf } from "./source-ref.mjs";

function repo(t) {
  const dir = mkdtempSync(join(tmpdir(), "source-ref-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const git = (...args) =>
    execFileSync("git", ["-c", "user.name=Test", "-c", "user.email=test@example.com", "-c", "init.defaultBranch=main", ...args], {
      cwd: dir,
      encoding: "utf8",
    }).trim();
  git("init", "-q");
  git("commit", "-q", "--allow-empty", "-m", "one");
  mkdirSync(join(dir, "apps", "showcase"), { recursive: true });
  return { dir, git, head: () => git("rev-parse", "HEAD") };
}

test("a branch's commit, from a folder deep inside the checkout", (t) => {
  const r = repo(t);
  assert.equal(commitOf(join(r.dir, "apps", "showcase")), r.head());
});

test("a detached HEAD, as a clone of a tag has", (t) => {
  const r = repo(t);
  r.git("commit", "-q", "--allow-empty", "-m", "two");
  r.git("tag", "v1.2.3-rc.1");
  r.git("checkout", "-q", "--detach", "v1.2.3-rc.1");
  assert.equal(commitOf(r.dir), r.head());
});

test("a branch whose ref is only in packed-refs", (t) => {
  const r = repo(t);
  r.git("pack-refs", "--all");
  assert.equal(commitOf(r.dir), r.head());
});

test("a linked worktree, whose .git is a file", (t) => {
  const r = repo(t);
  r.git("commit", "-q", "--allow-empty", "-m", "two");
  const other = join(r.dir, "..", `${join(r.dir).split("/").pop()}-wt`);
  t.after(() => rmSync(other, { recursive: true, force: true }));
  r.git("worktree", "add", "-q", "-b", "task", other, "HEAD~1");
  assert.equal(commitOf(other), r.git("rev-parse", "HEAD~1"));
});

test("nothing outside a checkout", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "source-ref-none-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  assert.equal(commitOf(dir), null);
});
