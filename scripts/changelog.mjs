import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const GROUPS = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"];

const sectionName = (line) => /^## \[([^\]]+)\]/.exec(line)?.[1];
const linkName = (line) => /^\[([^\]]+)\]: /.exec(line)?.[1];
const isStable = (version) => !version.includes("-");

const trim = (lines) => {
  const first = lines.findIndex((line) => line.trim());
  if (first === -1) return [];
  const last = lines.findLastIndex((line) => line.trim());
  return lines.slice(first, last + 1);
};

const concat = (newer, older) => {
  if (!newer.length) return older;
  if (!older.length) return newer;
  const oneList = older[0].startsWith("- ") && /^(- |\s+\S)/.test(newer.at(-1));
  return oneList ? [...newer, ...older] : [...newer, "", ...older];
};

const paragraphs = (blocks) => blocks.filter((block) => block.length).flatMap((block, i) => (i ? ["", ...block] : block));

const parse = (text) => {
  const lines = text.split("\n");
  const start = lines.findIndex(sectionName);
  const end = lines.findIndex((line, i) => i > start && linkName(line));
  const sections = [];
  for (const line of lines.slice(start, end === -1 ? undefined : end)) {
    if (sectionName(line)) sections.push({ name: sectionName(line), heading: line, body: [] });
    else sections.at(-1).body.push(line);
  }
  return {
    head: trim(lines.slice(0, start)),
    sections: sections.map((section) => ({ ...section, body: trim(section.body) })),
    links: end === -1 ? [] : trim(lines.slice(end)),
  };
};

const format = ({ head, sections, links }) => {
  const blocks = [head, ...sections.map(({ heading, body }) => paragraphs([[heading], body])), links];
  return `${paragraphs(blocks).join("\n")}\n`;
};

const outline = (body) => {
  const parts = [{ group: null, topic: null, lines: [] }];
  for (const line of body) {
    const group = /^### (.+)/.exec(line)?.[1].trim();
    const topic = /^#### (.+)/.exec(line)?.[1].trim();
    const current = parts.at(-1);
    if (group) parts.push({ group, topic: null, lines: [] });
    else if (topic && current.group) parts.push({ group: current.group, topic, lines: [] });
    else current.lines.push(line);
  }
  return parts;
};

const fold = (bodies) => {
  let intro = [];
  const groups = new Map();
  for (const { group, topic, lines } of bodies.flatMap(outline)) {
    if (!group) {
      intro = concat(intro, trim(lines));
      continue;
    }
    if (!groups.has(group)) groups.set(group, { lines: [], topics: new Map() });
    const entry = groups.get(group);
    if (topic) entry.topics.set(topic, concat(entry.topics.get(topic) ?? [], trim(lines)));
    else entry.lines = concat(entry.lines, trim(lines));
  }
  const order = [...GROUPS.filter((group) => groups.has(group)), ...[...groups.keys()].filter((group) => !GROUPS.includes(group))];
  const rendered = order.flatMap((group) => {
    const { lines, topics } = groups.get(group);
    const filled = [...topics].filter(([, topicLines]) => topicLines.length);
    if (!lines.length && !filled.length) return [];
    return [paragraphs([[`### ${group}`], lines, ...filled.flatMap(([topic, topicLines]) => [[`#### ${topic}`], topicLines])])];
  });
  return paragraphs([intro, ...rendered]);
};

const folded = (version) => (name) => name === "Unreleased" || (isStable(version) && name.startsWith(`${version}-`));

export function releasable(text, version) {
  const { sections } = parse(text);
  if (sections.some(({ name }) => name === version)) return true;
  return fold(sections.filter(({ name }) => folded(version)(name)).map(({ body }) => body)).length > 0;
}

export function release(text, version, date) {
  const changelog = parse(text);
  if (changelog.sections.some(({ name }) => name === version)) return text;
  const folds = folded(version);
  const unreleased = changelog.links.find((line) => linkName(line) === "Unreleased");
  const base = unreleased && /^\[Unreleased\]: (.+)\/compare\//.exec(unreleased)?.[1];
  if (!base) throw new Error("CHANGELOG.md has no [Unreleased] compare link");
  const body = fold(changelog.sections.filter(({ name }) => folds(name)).map((section) => section.body));
  const sections = [
    { heading: "## [Unreleased]", body: [] },
    { heading: `## [${version}] - ${date}`, body },
    ...changelog.sections.filter(({ name }) => !folds(name)),
  ];
  const links = changelog.links.flatMap((line) => {
    if (line === unreleased) return [`[Unreleased]: ${base}/compare/v${version}...HEAD`, `[${version}]: ${base}/releases/tag/v${version}`];
    return linkName(line) && folds(linkName(line)) ? [] : [line];
  });
  return format({ head: changelog.head, sections, links });
}

export function notes(text, version) {
  const section = parse(text).sections.find(({ name }) => name === version);
  if (!section) throw new Error(`CHANGELOG.md has no section for ${version}`);
  return section.body.join("\n");
}

const file = join(dirname(fileURLToPath(import.meta.url)), "..", "CHANGELOG.md");
const commands = {
  check: (version) => {
    if (!releasable(readFileSync(file, "utf8"), version)) process.exitCode = 1;
  },
  release: (version, date) => {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(date ?? "")) throw new Error("release needs a date like 2026-10-16");
    writeFileSync(file, release(readFileSync(file, "utf8"), version, date));
  },
  notes: (version) => process.stdout.write(`${notes(readFileSync(file, "utf8"), version)}\n`),
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command, version, ...rest] = process.argv.slice(2);
  if (!commands[command] || !version) {
    console.error("usage: node scripts/changelog.mjs check|release|notes <version> [date]");
    process.exit(2);
  }
  commands[command](version, ...rest);
}
