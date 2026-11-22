import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
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

test("a .git file naming its folder by a relative path, as a submodule has", (t) => {
  const r = repo(t);
  const outer = mkdtempSync(join(tmpdir(), "source-ref-sub-"));
  t.after(() => rmSync(outer, { recursive: true, force: true }));
  mkdirSync(join(outer, "modules"));
  r.git("clone", "-q", "--separate-git-dir", join(outer, "modules", "kaeso"), r.dir, join(outer, "kaeso"));
  writeFileSync(join(outer, "kaeso", ".git"), "gitdir: ../modules/kaeso\n");
  assert.equal(commitOf(join(outer, "kaeso")), r.head());
});

test("a shallow clone of an annotated tag, as a release build makes", (t) => {
  const r = repo(t);
  r.git("commit", "-q", "--allow-empty", "-m", "two");
  r.git("tag", "-a", "-m", "Release", "v1.2.3");
  const into = mkdtempSync(join(tmpdir(), "source-ref-tag-"));
  t.after(() => rmSync(into, { recursive: true, force: true }));
  r.git("clone", "-q", "--depth", "1", "--branch", "v1.2.3", `file://${r.dir}`, join(into, "src"));
  assert.equal(commitOf(join(into, "src", "apps", "showcase")), r.head());
});

test("nothing outside a checkout", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "source-ref-none-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  assert.equal(commitOf(dir), null);
});
