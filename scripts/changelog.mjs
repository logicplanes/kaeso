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

const leading = (lines) =>
  Math.max(
    0,
    lines.findIndex((line) => line.trim()),
  );

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
  for (const [i, line] of lines.slice(start, end === -1 ? undefined : end).entries()) {
    if (sectionName(line)) sections.push({ name: sectionName(line), heading: line, body: [], line: start + i + 1 });
    else sections.at(-1).body.push(line);
  }
  return {
    head: trim(lines.slice(0, start)),
    sections: sections.map((section) => ({ ...section, bodyLine: section.line + 1 + leading(section.body), body: trim(section.body) })),
    links: end === -1 ? [] : trim(lines.slice(end)),
    linksLine: end === -1 ? null : end + 1 + leading(lines.slice(end)),
  };
};

const format = ({ head, sections, links }) => {
  const blocks = [head, ...sections.map(({ heading, body }) => paragraphs([[heading], body])), links];
  return `${paragraphs(blocks).join("\n")}\n`;
};

const outline = (body, bodyLine = 1) => {
  const parts = [{ group: null, topic: null, lines: [], line: bodyLine }];
  for (const [i, line] of body.entries()) {
    const group = /^### (.+)/.exec(line)?.[1].trim();
    const topic = /^#### (.+)/.exec(line)?.[1].trim();
    const current = parts.at(-1);
    if (group) parts.push({ group, topic: null, lines: [], line: bodyLine + i + 1 });
    else if (topic && current.group) parts.push({ group: current.group, topic, lines: [], line: bodyLine + i + 1 });
    else current.lines.push(line);
  }
  return parts;
};

const fold = (bodies) => {
  let intro = [];
  const groups = new Map();
  for (const { group, topic, lines } of bodies.flatMap((body) => outline(body))) {
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

const unreleasedLink = (links) => links.find((line) => linkName(line) === "Unreleased");
const repositoryOf = (links) => /^\[Unreleased\]: (.+)\/compare\//.exec(unreleasedLink(links) ?? "")?.[1] ?? null;

const TYPES = { added: "Added", changed: "Changed", deprecated: "Deprecated", removed: "Removed", fixed: "Fixed", security: "Security" };
const PRECEDENCE = ["security", "added", "changed", "deprecated", "removed", "fixed"];
const INFERRED = [
  [["add", "introduce"], "added"],
  [["deprecate"], "deprecated"],
  [["remove", "drop"], "removed"],
  [["fix"], "fixed"],
];
const INTERNAL = "internal";
const BREAKING = "breaking";
const type = (name, color, note = "") => ({ name, color, description: `Release notes: under ${TYPES[name]}${note}` });
const area = (topic) => ({ name: `area: ${topic.toLowerCase()}`, color: "c5def5", description: `Release notes: under the ${topic} topic`, topic });

export const LABELS = [
  { name: BREAKING, color: "b60205", description: "Release notes: also named at the top as a breaking change" },
  type("added", "0e8a16"),
  type("changed", "fbca04", ", the default"),
  type("deprecated", "d4c5f9"),
  type("removed", "e99695"),
  type("fixed", "1d76db"),
  type("security", "d93f0b"),
  { name: INTERNAL, color: "ededed", description: "Release notes: left out, such as CI, tests and tooling" },
  ...["Gateway", "KPP", "Console", "UI library", "Documentation", "Self-hosting"].map(area),
];

const AREAS = LABELS.filter(({ topic }) => topic).map(({ name, topic }) => [name, topic]);
const DEPENDABOT = "dependabot[bot]";
const DEPENDENCIES = "dependencies";

const typeOf = ({ title, labels }) => {
  const labelled = PRECEDENCE.find((type) => labels.includes(type));
  if (labelled) return labelled;
  const word = /^\p{L}+/u.exec(title)?.[0].toLowerCase() ?? "";
  return INFERRED.find(([words]) => words.includes(word))?.[1] ?? "changed";
};

const topicOf = ({ labels }) => {
  const areas = AREAS.filter(([label]) => labels.includes(label));
  return areas.length === 1 ? areas[0][1] : null;
};

const isDependency = ({ author, labels }) => author === DEPENDABOT || labels.includes(DEPENDENCIES);

const escaped = (text) =>
  text
    .split(/(`[^`]*`)/)
    .map((part, i) => (i % 2 ? part : part.replace(/[\\<>[\]]/g, "\\$&")))
    .join("")
    .replace(/^(\d+)([.)])/, "$1\\$2")
    .replace(/^(#|[-+*](?=\s))/, "\\$1");

const link = ({ number, url }) => `[#${number}](${url})`;
const sentence = (pull) => `${escaped(pull.title.replace(/\.+$/, ""))} (${link(pull)})`;
const item = (pull) => `- ${sentence(pull)}`;

const checked = (pulls) => {
  if (!Array.isArray(pulls)) throw new Error("the pull requests must be a JSON array");
  return pulls.map((pull, i) => {
    const where = `pull request ${i + 1}`;
    if (!Number.isInteger(pull?.number) || pull.number < 1) throw new Error(`${where} has no number`);
    if (typeof pull.title !== "string" || !pull.title.trim()) throw new Error(`#${pull.number} has no title`);
    if (typeof pull.url !== "string" || !/^https:\/\/\S+$/.test(pull.url)) throw new Error(`#${pull.number} has no https address`);
    if (!Array.isArray(pull.labels) || pull.labels.some((label) => typeof label !== "string")) throw new Error(`#${pull.number} has no list of labels`);
    return {
      number: pull.number,
      title: pull.title.replace(/\s+/g, " ").trim(),
      url: pull.url,
      labels: pull.labels,
      author: typeof pull.author === "string" ? pull.author : "",
    };
  });
};

export function generate(pulls) {
  const unique = [...new Map(checked(pulls).map((pull) => [pull.number, pull])).values()].toSorted((a, b) => a.number - b.number);
  const visible = unique.filter(({ labels }) => !labels.includes(INTERNAL));
  const own = visible.filter((pull) => !isDependency(pull));
  const dependencies = visible.filter(isDependency);
  const entries = own.map((pull) => ({ group: TYPES[typeOf(pull)], topic: topicOf(pull), line: item(pull) }));
  if (dependencies.length) entries.push({ group: TYPES.changed, topic: null, line: `- Dependencies updated (${dependencies.map(link).join(", ")})` });
  const intro = own.filter(({ labels }) => labels.includes(BREAKING)).map((pull) => `- **Breaking:** ${sentence(pull)}`);
  const sections = GROUPS.flatMap((group) => {
    const inGroup = entries.filter((entry) => entry.group === group);
    if (!inGroup.length) return [];
    const lines = (topic) => inGroup.filter((entry) => entry.topic === topic).map((entry) => entry.line);
    const topics = AREAS.map(([, topic]) => [topic, lines(topic)]).filter(([, topicLines]) => topicLines.length);
    return [paragraphs([[`### ${group}`], lines(null), ...topics.flatMap(([topic, topicLines]) => [[`#### ${topic}`], topicLines])])];
  });
  return paragraphs([intro, ...sections]);
}

const UNRELEASED = "Unreleased";

const drafted = (changelog, pulls) => {
  const pending = changelog.sections.find(({ name }) => name === UNRELEASED);
  if (pending && fold([pending.body]).length) return changelog.sections;
  const body = generate(pulls);
  if (!pending) return [{ name: UNRELEASED, heading: `## [${UNRELEASED}]`, body }, ...changelog.sections];
  return changelog.sections.map((section) => (section === pending ? { ...section, body } : section));
};

export function releasable(text, version, pulls = []) {
  const changelog = parse(text);
  if (changelog.sections.some(({ name }) => name === version)) return true;
  return (
    fold(
      drafted(changelog, pulls)
        .filter(({ name }) => folded(version)(name))
        .map(({ body }) => body),
    ).length > 0
  );
}

export function release(text, version, date, pulls = []) {
  const changelog = parse(text);
  if (changelog.sections.some(({ name }) => name === version)) return text;
  const folds = folded(version);
  const unreleased = unreleasedLink(changelog.links);
  const base = repositoryOf(changelog.links);
  if (!base) throw new Error("CHANGELOG.md has no [Unreleased] compare link");
  const drafts = drafted(changelog, pulls);
  const body = fold(drafts.filter(({ name }) => folds(name)).map((section) => section.body));
  const sections = [{ heading: `## [${UNRELEASED}]`, body: [] }, { heading: `## [${version}] - ${date}`, body }, ...drafts.filter(({ name }) => !folds(name))];
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

const slug = (text) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-+|-+$/g, "");

const entry = (id, body, bodyLine) => {
  const seen = new Map();
  const unique = (base) => {
    const count = seen.get(base) ?? 0;
    seen.set(base, count + 1);
    return count ? `${base}-${count + 1}` : base;
  };
  const markdown = (lines) => trim(lines).join("\n");
  const startOf = (lines, line) => line + leading(lines);
  const [intro, ...parts] = outline(body, bodyLine);
  const sections = [];
  for (const { group, topic, lines, line } of parts) {
    if (!topic) sections.push({ name: group, id: unique(`${id}-${slug(group)}`), markdown: markdown(lines), line: startOf(lines, line), topics: [] });
    else {
      const section = sections.at(-1);
      section.topics.push({ name: topic, id: unique(`${section.id}-${slug(topic)}`), markdown: markdown(lines), line: startOf(lines, line) });
    }
  }
  const filled = sections
    .map((section) => ({ ...section, topics: section.topics.filter((topic) => topic.markdown) }))
    .filter((section) => section.markdown || section.topics.length);
  return { id, intro: markdown(intro.lines), introLine: startOf(intro.lines, intro.line), sections: filled };
};

const isEmpty = ({ intro, sections }) => !intro && !sections.length;

const RELEASE_HEADING = /^## \[([^\]]+)\](?:\s+-\s+(\d{4}-\d{2}-\d{2}))?(\s+\[YANKED\])?\s*$/i;

export function history(text) {
  if (!text.split("\n").some(sectionName)) return { repository: null, definitions: "", definitionsLine: null, unreleased: null, releases: [] };
  const { sections, links, linksLine } = parse(text);
  const repository = repositoryOf(links);
  const versions = sections
    .filter(({ name }) => name !== "Unreleased")
    .map(({ name, heading, body, bodyLine }) => {
      const [, , date, yanked] = RELEASE_HEADING.exec(heading) ?? [];
      return { ...entry(`v${name}`, body, bodyLine), version: name, date: date ?? null, prerelease: !isStable(name), yanked: Boolean(yanked) };
    });
  const latest = versions.find(({ prerelease, yanked }) => !prerelease && !yanked)?.version ?? null;
  const releases = versions.map((release, i) => {
    const previous = versions[i + 1]?.version ?? null;
    return {
      ...release,
      latest: release.version === latest,
      previous,
      url: repository && `${repository}/releases/tag/v${release.version}`,
      compare: repository && previous && `${repository}/compare/v${previous}...v${release.version}`,
    };
  });
  const pending = sections.find(({ name }) => name === "Unreleased");
  const unreleased = pending && entry("unreleased", pending.body, pending.bodyLine);
  const since = releases[0]?.version ?? null;
  const compare = /^\[Unreleased\]: (\S+)/.exec(unreleasedLink(links) ?? "")?.[1] ?? null;
  return {
    repository,
    definitions: links.filter(linkName).join("\n"),
    definitionsLine: linksLine,
    unreleased: unreleased && !isEmpty(unreleased) ? { ...unreleased, since, compare: since && compare } : null,
    releases,
  };
}

const file = join(dirname(fileURLToPath(import.meta.url)), "..", "CHANGELOG.md");
const pullsFrom = (path) => (path ? JSON.parse(readFileSync(path, "utf8")) : []);
const commands = {
  check: (version, pulls) => {
    if (!releasable(readFileSync(file, "utf8"), version, pullsFrom(pulls))) process.exitCode = 1;
  },
  release: (version, date, pulls) => {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(date ?? "")) throw new Error("release needs a date like 2026-10-16");
    writeFileSync(file, release(readFileSync(file, "utf8"), version, date, pullsFrom(pulls)));
  },
  notes: (version) => process.stdout.write(`${notes(readFileSync(file, "utf8"), version)}\n`),
  draft: (pulls) => process.stdout.write(`${generate(pullsFrom(pulls)).join("\n")}\n`),
  labels: () => process.stdout.write(LABELS.map(({ name, color, description }) => `${name}\t${color}\t${description}\n`).join("")),
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command, ...rest] = process.argv.slice(2);
  if (!commands[command] || rest.length < Math.min(1, commands[command].length)) {
    console.error(
      "usage: node scripts/changelog.mjs check <version> [pulls.json] | release <version> <date> [pulls.json] | notes <version> | draft <pulls.json> | labels",
    );
    process.exit(2);
  }
  commands[command](...rest);
}
