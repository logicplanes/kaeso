import { test } from "node:test";
import assert from "node:assert/strict";
import {
  atLeast,
  checkConsolePort,
  consolePort,
  checkDisk,
  checkDocker,
  checkNode,
  checkPnpm,
  checkPorts,
  checkTool,
  parseArgs,
  parseVersion,
  report,
  stackPorts,
} from "./bootstrap.mjs";

test("options", () => {
  assert.deepEqual(parseArgs([]), { check: false, all: false, seed: true, help: false });
  assert.deepEqual(parseArgs(["--check", "--all", "--no-seed"]), { check: true, all: true, seed: false, help: false });
  assert.equal(parseArgs(["-h"]).help, true);
  assert.throws(() => parseArgs(["--fast"]), /Unknown option --fast/);
});

test("versions are read from what the tools print and compared part by part", () => {
  assert.deepEqual(parseVersion("v24.21.0"), [24, 21, 0]);
  assert.deepEqual(parseVersion("Docker Compose version v2.40.3-desktop.1"), [2, 40, 3]);
  assert.deepEqual(parseVersion("2.24"), [2, 24, 0]);
  assert.equal(parseVersion("unknown"), null);
  assert.equal(atLeast([2, 24, 0], [2, 24]), true);
  assert.equal(atLeast([2, 23, 9], [2, 24]), false);
  assert.equal(atLeast([3, 0, 0], [2, 24]), true);
});

test("Node.js must be the major version the repository pins and at least the engines minimum", () => {
  assert.equal(checkNode("24.21.0", "24.21.0", ">=24.12.0").level, "ok");
  assert.equal(checkNode("24.13.1", "24.21.0", ">=24.12.0").level, "ok");
  const old = checkNode("24.10.0", "24.21.0", ">=24.12.0");
  assert.equal(old.level, "error");
  assert.match(old.detail, /24\.10\.0 runs, and the repository needs 24\.21\.0/);
  assert.equal(checkNode("22.11.0", "24.21.0", ">=24.12.0").level, "error");
  assert.equal(checkNode("26.0.0", "24.21.0", ">=24.12.0").level, "error");
});

test("pnpm must be the version package.json pins, which Corepack provides", () => {
  assert.equal(checkPnpm("11.21.0", "pnpm@11.21.0").level, "ok");
  assert.equal(checkPnpm("11.21.0", "pnpm@11.21.0+sha512.abc").level, "ok");
  const other = checkPnpm("10.4.1", "pnpm@11.21.0");
  assert.equal(other.level, "error");
  assert.match(other.fix, /corepack enable/);
  assert.match(checkPnpm(null, "pnpm@11.21.0").fix, /corepack enable/);
});

test("a missing tool says how to install it", () => {
  assert.equal(checkTool("Git", "git version 2.50.1", "Install Git.").level, "ok");
  assert.deepEqual(checkTool("rustup", null, "Install rustup."), { name: "rustup", level: "error", detail: "isn't installed", fix: "Install rustup." });
});

test("Docker must answer with Compose 2.24 or newer, and enough memory to build the gateway", () => {
  assert.equal(checkDocker(null, null)[0].level, "error");
  const good = checkDocker({ ServerVersion: "29.0.1", MemTotal: 16e9 }, "2.40.3");
  assert.deepEqual(
    good.map((r) => r.level),
    ["ok", "ok"],
  );
  const oldCompose = checkDocker({ ServerVersion: "24.0.7", MemTotal: 16e9 }, "2.20.2");
  assert.equal(oldCompose[1].level, "error");
  assert.match(oldCompose[1].detail, /2\.20\.2 is older than 2\.24/);
  assert.equal(checkDocker({ ServerVersion: "29.0.1" }, null)[1].level, "error");
  const small = checkDocker({ ServerVersion: "29.0.1", MemTotal: 2e9 }, "2.40.3");
  assert.equal(small.at(-1).level, "warn");
  assert.match(small.at(-1).detail, /2\.0 GB/);
  assert.match(small.at(-1).fix, /colima start --memory 8/);
});

test("little free disk is a warning", () => {
  assert.equal(checkDisk(500e9).level, "ok");
  assert.equal(checkDisk(12e9).level, "warn");
  assert.match(checkDisk(12e9).detail, /12 GB free/);
});

test("ports taken by another program fail and name the variable that moves them", () => {
  const ports = stackPorts({
    postgresPort: 55432,
    clickhousePort: 58123,
    valkeyPort: 56379,
    mailPort: 58025,
    smtpPort: 51025,
    gatewayPort: 18080,
    idpPort: 18091,
    releasesPort: 18092,
    accountsPort: 18093,
  });
  assert.equal(ports.length, 9);
  assert.equal(checkPorts(ports, new Set(), new Set()).level, "ok");
  assert.equal(checkPorts(ports, new Set([55432, 18080]), new Set([55432, 18080])).level, "ok", "the stack's own ports are fine");
  const clash = checkPorts(ports, new Set([55432, 18080]), new Set([18080]));
  assert.equal(clash.level, "error");
  assert.match(clash.detail, /55432 \(KAESO_POSTGRES_PORT\)/);
  assert.doesNotMatch(clash.detail, /18080/);
  assert.match(clash.fix, /KAESO_POSTGRES_PORT=56432 pnpm bootstrap/);
});

test("the console port comes from the stack's public address", () => {
  assert.equal(consolePort("http://localhost:3000"), 3000);
  assert.equal(consolePort("http://localhost"), 80);
  assert.equal(consolePort("https://kaeso.localhost"), 443);
});

test("a taken console port is a warning with the way around it", () => {
  assert.equal(checkConsolePort(3000, new Set(), new Set()).level, "ok");
  const taken = checkConsolePort(3000, new Set([3000]), new Set());
  assert.equal(taken.level, "warn");
  assert.match(taken.fix, /KAESO_PUBLIC_URL=http:\/\/localhost:3010 pnpm bootstrap, then KAESO_PUBLIC_URL=http:\/\/localhost:3010 pnpm dev --port 3010/);
  assert.match(checkConsolePort(3010, new Set([3010]), new Set()).fix, /localhost:3020/);
});

test("the report shows each check with its fix under it", () => {
  const text = report([
    { name: "Node.js", level: "ok", detail: "24.21.0" },
    { name: "pnpm", level: "error", detail: "isn't installed", fix: "Run corepack enable." },
  ]);
  assert.equal(text, "ok    Node.js: 24.21.0\nfail  pnpm: isn't installed\n      Run corepack enable.");
});
