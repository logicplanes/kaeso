import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readFileSync, statfsSync } from "node:fs";
import { connect } from "node:net";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

export const MIN_COMPOSE = [2, 24];
export const MIN_DOCKER_MEMORY = 8e9;
export const MIN_FREE_DISK = 40e9;
export const CARGO_TOOLS = { "cargo-deny": "deny", "cargo-llvm-cov": "llvm-cov", "cargo-nextest": "nextest", "sqlx-cli": "sqlx" };

export const USAGE = `Sets up a Kaeso checkout for development: checks what it needs, installs the dependencies, starts the
development stack and fills it with sample data.

Usage: pnpm bootstrap [options]

  --check     only check the prerequisites and the ports
  --all       also install the browsers, the Cargo tools and the Python packages the full test suites need
  --no-seed   start the stack without sample data, to set Kaeso up by hand
  -h, --help  show this help
`;

export function parseArgs(argv) {
  const options = { check: false, all: false, seed: true, help: false };
  for (const arg of argv) {
    if (arg === "--check") options.check = true;
    else if (arg === "--all") options.all = true;
    else if (arg === "--no-seed") options.seed = false;
    else if (arg === "-h" || arg === "--help") options.help = true;
    else throw new Error(`Unknown option ${arg}. Run pnpm bootstrap --help to see the options.`);
  }
  return options;
}

export function parseVersion(text) {
  const match = /(\d+)\.(\d+)(?:\.(\d+))?/.exec(text ?? "");
  return match ? [Number(match[1]), Number(match[2]), Number(match[3] ?? 0)] : null;
}

export function atLeast(version, minimum) {
  for (let i = 0; i < minimum.length; i++) {
    const have = version[i] ?? 0;
    if (have !== minimum[i]) return have > minimum[i];
  }
  return true;
}

const pass = (name, detail) => ({ name, level: "ok", detail });
const warn = (name, detail, fix) => ({ name, level: "warn", detail, fix });
const fail = (name, detail, fix) => ({ name, level: "error", detail, fix });

export function checkNode(running, pinned, engines) {
  const have = parseVersion(running);
  const want = parseVersion(pinned);
  const minimum = parseVersion(engines);
  if (!have || !want) return fail("Node.js", `can't read the version ${running}`, "Install Node.js from https://nodejs.org.");
  if (have[0] !== want[0] || (minimum && !atLeast(have, minimum)))
    return fail(
      "Node.js",
      `${running} runs, and the repository needs ${pinned} (.node-version)`,
      `Install Node.js ${pinned}, for example with fnm install or nvm install.`,
    );
  return pass("Node.js", running);
}

export function checkPnpm(running, packageManager) {
  const want = packageManager?.split("@")[1]?.split("+")[0];
  if (!running) return fail("pnpm", "isn't installed", "Run corepack enable; Corepack comes with Node.js.");
  if (want && running !== want)
    return fail("pnpm", `${running} runs, and package.json pins ${want}`, "Run corepack enable, so the pinned pnpm runs in this checkout.");
  return pass("pnpm", running);
}

export function checkTool(name, version, fix) {
  return version ? pass(name, version) : fail(name, "isn't installed", fix);
}

export function checkDocker(info, composeVersion) {
  if (!info) return [fail("Docker", "doesn't answer", "Start Docker Desktop, OrbStack or Colima, or the Docker service, and run this again.")];
  const results = [pass("Docker", info.ServerVersion ?? "running")];
  const compose = parseVersion(composeVersion);
  if (!compose) results.push(fail("Docker Compose", "isn't installed", "Install the Compose plugin: https://docs.docker.com/compose/install/"));
  else if (!atLeast(compose, MIN_COMPOSE))
    results.push(fail("Docker Compose", `${composeVersion} is older than ${MIN_COMPOSE.join(".")}`, "Update Docker or its Compose plugin."));
  else results.push(pass("Docker Compose", composeVersion));
  const memory = Number(info.MemTotal ?? 0);
  if (memory > 0 && memory < MIN_DOCKER_MEMORY * 0.95)
    results.push(
      warn(
        "Docker memory",
        `${(memory / 1e9).toFixed(1)} GB, and building the gateway in Docker needs about 8 GB`,
        "Give Docker more memory in its settings, or with colima start --memory 8 on Colima.",
      ),
    );
  return results;
}

export function checkDisk(freeBytes) {
  const gb = (freeBytes / 1e9).toFixed(0);
  return freeBytes < MIN_FREE_DISK
    ? warn(
        "Disk",
        `${gb} GB free, and the Rust builds and test databases take tens of GB`,
        "Free some space, or remove target/debug/incremental from earlier builds.",
      )
    : pass("Disk", `${gb} GB free`);
}

export function stackPorts(stack) {
  return [
    ["KAESO_POSTGRES_PORT", stack.postgresPort],
    ["KAESO_CLICKHOUSE_PORT", stack.clickhousePort],
    ["KAESO_VALKEY_PORT", stack.valkeyPort],
    ["KAESO_MAIL_PORT", stack.mailPort],
    ["KAESO_MAIL_SMTP_PORT", stack.smtpPort],
    ["KAESO_GATEWAY_PORT", stack.gatewayPort],
    ["KAESO_IDP_PORT", stack.idpPort],
    ["KAESO_RELEASES_PORT", stack.releasesPort],
    ["KAESO_ACCOUNTS_PORT", stack.accountsPort],
  ];
}

export function checkPorts(ports, taken, ours) {
  const clashes = ports.filter(([, port]) => taken.has(port) && !ours.has(port));
  if (!clashes.length) return pass("Ports", "free, or used by this development stack");
  const list = clashes.map(([name, port]) => `${port} (${name})`).join(", ");
  return fail(
    "Ports",
    `taken by another program: ${list}`,
    `Stop that program, or move the stack: set the variable to a free port, such as ${clashes[0][0]}=${clashes[0][1] + 1000} pnpm bootstrap.`,
  );
}

export function consolePort(publicUrl) {
  const url = new URL(publicUrl);
  return Number(url.port || (url.protocol === "https:" ? 443 : 80));
}

export function checkConsolePort(port, taken, ours) {
  if (!taken.has(port) || ours.has(port)) return pass("Console port", `${port} is free`);
  const other = port === 3010 ? 3020 : 3010;
  return warn(
    "Console port",
    `${port} is taken by another program`,
    `Stop it before pnpm dev, or move the console: KAESO_PUBLIC_URL=http://localhost:${other} pnpm bootstrap, then KAESO_PUBLIC_URL=http://localhost:${other} pnpm dev --port ${other}.`,
  );
}

export function report(results) {
  const mark = { ok: "ok  ", warn: "warn", error: "fail" };
  const lines = results.map((r) => `${mark[r.level]}  ${r.name}: ${r.detail}${r.fix ? `\n      ${r.fix}` : ""}`);
  return lines.join("\n");
}

function output(command, args) {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
  return result.status === 0 ? result.stdout.trim() : null;
}

function listening(port) {
  return new Promise((resolve) => {
    const socket = connect({ host: "127.0.0.1", port, timeout: 500 });
    const done = (answer) => {
      socket.destroy();
      resolve(answer);
    };
    socket.once("connect", () => done(true));
    socket.once("timeout", () => done(false));
    socket.once("error", () => done(false));
  });
}

function stackPublished(project) {
  const text = output("docker", ["ps", "--filter", `label=com.docker.compose.project=${project}`, "--format", "{{.Ports}}"]) ?? "";
  return new Set([...text.matchAll(/:(\d+)->/g)].map((m) => Number(m[1])));
}

async function checks(stack, options) {
  const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  const pinned = readFileSync(join(root, ".node-version"), "utf8").trim();
  const results = [
    checkNode(process.versions.node, pinned, pkg.engines?.node),
    checkPnpm(output("pnpm", ["--version"]), pkg.packageManager),
    checkTool("Git", output("git", ["--version"]), "Install Git: https://git-scm.com."),
    checkTool("rustup", output("rustup", ["--version"]), "Install rustup from https://rustup.rs; it installs the pinned Rust toolchain itself."),
  ];
  const infoText = output("docker", ["info", "--format", "{{json .}}"]);
  let info = null;
  try {
    info = infoText ? JSON.parse(infoText) : null;
  } catch {
    info = null;
  }
  results.push(...checkDocker(info, output("docker", ["compose", "version", "--short"])));
  if (options.all) {
    const python = output("python3", ["--version"]);
    const version = parseVersion(python);
    results.push(
      version && atLeast(version, [3, 13])
        ? pass("Python", python)
        : fail("Python", python ? `${python} is older than 3.13` : "isn't installed", "Install Python 3.13 for the end-to-end tests: https://www.python.org."),
    );
  }
  results.push(checkDisk(Number(statfsSync(root).bavail) * Number(statfsSync(root).bsize)));
  const ports = stackPorts(stack);
  const console = consolePort(stack.publicUrl);
  const all = [...ports.map(([, port]) => port), console];
  const open = await Promise.all(all.map(async (port) => [port, await listening(port)]));
  const taken = new Set(open.filter(([, yes]) => yes).map(([port]) => port));
  const ours = info ? stackPublished(stack.project) : new Set();
  results.push(checkPorts(ports, taken, ours), checkConsolePort(console, taken, ours));
  return results;
}

function run(title, command, args, env = {}) {
  console.log(`\n==> ${title}`);
  execFileSync(command, args, { cwd: root, stdio: "inherit", env: { ...process.env, ...env } });
}

async function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exit(2);
  }
  if (options.help) return void process.stdout.write(USAGE);
  const { STACKS, setupToken } = await import(pathToFileURL(join(root, "apps/console/scripts/stack.mjs")).href);
  const stack = STACKS.dev;
  const results = await checks(stack, options);
  console.log(report(results));
  if (results.some((r) => r.level === "error")) {
    console.error("\nFix the problems marked fail, then run pnpm bootstrap again.");
    process.exit(1);
  }
  if (options.check) return;

  run("Installing the dependencies", "pnpm", ["install", "--frozen-lockfile"]);
  run("Building the UI library", "pnpm", ["build:packages"]);
  run("Starting the development stack; the first build of the gateway takes several minutes", "pnpm", ["stack:up"]);
  if (options.seed && setupToken(stack) !== null) run("Adding the sample data", "pnpm", ["seed"]);
  else if (options.seed) console.log("\n==> The stack is set up already, so its data stays. pnpm stack:down && pnpm bootstrap starts afresh.");
  if (options.all) {
    run("Installing the browsers for the end-to-end tests", "pnpm", ["--filter", "kaeso-console", "exec", "playwright", "install", "chromium"]);
    run("Installing WebKit for the showcase's tests", "pnpm", ["--filter", "kaeso-showcase", "exec", "playwright", "install", "webkit"]);
    const missing = Object.entries(CARGO_TOOLS)
      .filter(([, command]) => !output("cargo", [command, "--version"]))
      .map(([tool]) => tool);
    if (missing.length) run("Installing the Cargo tools", "cargo", ["install", "--locked", ...missing]);
    if (!existsSync(join(root, "e2e/.venv"))) run("Creating the Python environment for the end-to-end tests", "python3", ["-m", "venv", "e2e/.venv"]);
    run("Installing the Python packages", join(root, "e2e/.venv/bin/pip"), ["install", "--quiet", "-r", "e2e/requirements.txt"]);
  }
  console.log(`
Kaeso is ready for development. Serve the console with:

  pnpm dev

Then open ${stack.publicUrl}${options.seed ? " and sign in as owner@example.com with the password correct horse battery staple" : " with the setup link above"}. Emails arrive at http://127.0.0.1:${stack.mailPort}.`);
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) await main();
