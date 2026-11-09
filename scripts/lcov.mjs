// A line counts as covered when any shard ran it, as if every test had run in one process.

import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

export function add(files, text) {
  let lines = null;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (line.startsWith("SF:")) {
      const path = line.slice(3);
      lines = files.get(path) ?? new Map();
      files.set(path, lines);
    } else if (line.startsWith("DA:") && lines) {
      const [number, hits] = line.slice(3).split(",");
      lines.set(number, (lines.get(number) ?? 0) + Number(hits));
    } else if (line === "end_of_record") {
      lines = null;
    }
  }
  return files;
}

export function summary(files) {
  let total = 0;
  let covered = 0;
  for (const lines of files.values()) {
    for (const hits of lines.values()) {
      total += 1;
      if (hits > 0) covered += 1;
    }
  }
  return { total, covered, percent: total === 0 ? 100 : (covered / total) * 100 };
}

// The shards take the minimum and the ignored files from the `coverage-gateway` alias, the command people run locally.
export function gatewayAlias(config) {
  const alias = /^coverage-gateway = \[([^\]]*)\]/ms.exec(config);
  if (!alias) throw new Error("no coverage-gateway alias in .cargo/config.toml");
  const args = [...alias[1].matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((m) => JSON.parse(`"${m[1]}"`));
  const after = (flag) => {
    const index = args.indexOf(flag);
    if (index < 0 || index + 1 >= args.length) throw new Error(`coverage-gateway has no ${flag}`);
    return args[index + 1];
  };
  return { minimum: Number(after("--fail-under-lines")), ignore: after("--ignore-filename-regex") };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [command, ...reports] = process.argv.slice(2);
  const alias = gatewayAlias(readFileSync(new URL("../.cargo/config.toml", import.meta.url), "utf8"));
  if (command === "ignore") {
    console.log(alias.ignore);
  } else if (command === "check" && reports.length > 0) {
    const files = new Map();
    for (const report of reports) add(files, readFileSync(report, "utf8"));
    const { total, covered, percent } = summary(files);
    console.log(`Lines: ${covered} of ${total} covered (${percent.toFixed(2)}%) across ${reports.length} shards, ${files.size} files`);
    if (percent < alias.minimum) {
      console.error(`Line coverage ${percent.toFixed(2)}% is under the minimum of ${alias.minimum}%`);
      process.exit(1);
    }
  } else {
    console.error("usage: node scripts/lcov.mjs ignore | check <report.info>...");
    process.exit(2);
  }
}
