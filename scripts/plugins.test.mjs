import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { basename, dirname, extname, join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const json = (path) => JSON.parse(readFileSync(join(root, path), "utf8"));
const text = (path) => readFileSync(join(root, path), "utf8");
const version = json("package.json").version;
const KEBAB = /^[a-z0-9]+(-[a-z0-9]+)*$/;

// Claude Code installs plugins/kaeso from the plugin directory, Cursor installs plugins/kaeso-cursor.
const CLAUDE = "plugins/kaeso";
const CURSOR = "plugins/kaeso-cursor";

const files = (dir) =>
  readdirSync(join(root, dir), { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile() || entry.isSymbolicLink())
    .map((entry) => relative(root, join(entry.parentPath, entry.name)))
    .sort();

// The skill as Cursor ships it: Claude Code's, without the section that connects the server with `claude mcp add`.
const CONNECT = /^## Connecting Kaeso\n[\s\S]*?(?=^## )/m;
const skill = (path) => {
  const [, front = "", body = ""] = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/.exec(text(path)) ?? [];
  return { name: /^name: (.*)$/m.exec(front)?.[1], description: /^description: (.*)$/m.exec(front)?.[1] ?? "", body };
};

test("each marketplace lists its plugin's folder, and the folder's manifest carries the listed name", () => {
  for (const [file, manifest, folder] of [
    [".claude-plugin/marketplace.json", ".claude-plugin/plugin.json", CLAUDE],
    [".cursor-plugin/marketplace.json", ".cursor-plugin/plugin.json", CURSOR],
  ]) {
    const market = json(file);
    assert.match(market.name, KEBAB, file);
    assert.ok(market.owner?.name, file);
    assert.deepEqual(
      market.plugins.map((entry) => entry.source.replace(/^\.\//, "")),
      [folder],
      file,
    );
    for (const entry of market.plugins) {
      assert.match(entry.name, KEBAB, file);
      assert.equal(json(join(folder, manifest)).name, entry.name, `${file}: ${entry.name}`);
    }
  }
  assert.deepEqual(readdirSync(join(root, "plugins")).sort(), [CLAUDE, CURSOR].map((dir) => basename(dir)).sort());
});

for (const [dir, manifest] of [
  [CLAUDE, ".claude-plugin/plugin.json"],
  [CURSOR, ".cursor-plugin/plugin.json"],
]) {
  test(`${dir} ships the release's version, its license and a README`, () => {
    const plugin = json(`${dir}/${manifest}`);
    assert.equal(plugin.version, version);
    assert.equal(plugin.license, json("package.json").license);
    assert.equal(text(`${dir}/LICENSE`), text("LICENSE"));
    const words = text(`${dir}/README.md`).split(/\s+/).filter(Boolean);
    assert.ok(words.length >= 40, "the directories ask for a README of 40 words or more");
    assert.match(plugin.homepage, /^https:\/\//);
  });

  test(`${dir}'s skills say when to use them`, () => {
    for (const name of readdirSync(join(root, dir, "skills"))) {
      const { name: declared, description } = skill(`${dir}/skills/${name}/SKILL.md`);
      assert.equal(declared, name);
      assert.ok(description.length >= 40, name);
      assert.match(description, /^Use when /, name);
    }
  });
}

test("the Claude Code plugin has its icon, its links and no Cursor files", () => {
  const claude = json(`${CLAUDE}/.claude-plugin/plugin.json`);
  assert.match(claude.icon, /^\.\/[\w/-]+\.png$/);
  assert.deepEqual([...readFileSync(join(root, CLAUDE, claude.icon)).subarray(1, 4)], [...Buffer.from("PNG")]);
  for (const url of [claude.homepage, claude.documentationUrl, claude.supportUrl, claude.privacyPolicyUrl]) assert.match(url, /^https:\/\//);
  assert.equal(existsSync(join(root, CLAUDE, ".cursor-plugin")), false);
  assert.equal(existsSync(join(root, CURSOR, ".claude-plugin")), false);
});

test("the Claude Code plugin asks for the instance's address in Claude Code, and its skill connects every other Claude", () => {
  const claude = json(`${CLAUDE}/.claude-plugin/plugin.json`);
  assert.equal(claude.mcpServers, undefined);
  assert.equal(existsSync(join(root, CLAUDE, "mcp.json")), false);
  const servers = Object.values(json(`${CLAUDE}/.mcp.json`).mcpServers);
  assert.equal(servers.length, 1);
  for (const server of servers) {
    const key = /^\$\{user_config\.(\w+)\}$/.exec(server.url)?.[1];
    assert.ok(key, "the server's url is one user_config reference, the only variable the plugin directory accepts there");
    assert.equal(claude.userConfig[key].type, "string");
    assert.notEqual(claude.userConfig[key].required, true, "Cowork and claude.ai can't prompt for it");
  }
  const { description, body } = skill(`${CLAUDE}/skills/kaeso/SKILL.md`);
  assert.match(description, /connect/i);
  assert.equal(/^## .*$/m.exec(body)?.[0], "## Connecting Kaeso", "the skill connects the server before anything else");
  const section = CONNECT.exec(body)?.[0] ?? "";
  assert.match(section, /claude mcp add --transport http --scope user kaeso https:\/\/<their install>\/mcp/);
  assert.match(section, /`\/mcp`/);
  assert.match(section, /\/plugin configure kaeso@kaeso/);
  assert.match(section, /https:\/\/claude\.ai\/customize\/connectors\?modal=add-custom-connector&connectorName=Kaeso&connectorUrl=/);
});

test("the Cursor plugin asks for the instance's address and reaches its MCP server there", () => {
  const cursor = json(`${CURSOR}/.cursor-plugin/plugin.json`);
  assert.match(cursor.logo, /^[\w/-]+\.svg$/);
  assert.match(text(join(CURSOR, cursor.logo)), /^<svg [^>]*viewBox="0 0 512 512"/);
  const config = text(`${CURSOR}/mcp.json`);
  for (const server of Object.values(JSON.parse(config).mcpServers)) assert.match(server.url, /^\$\{\w+\}\/mcp$/);
  for (const name of [...config.matchAll(/\$\{(\w+)\}/g)].map((m) => m[1])) {
    assert.equal(cursor.variables.properties[name]?.type, "string", name);
    assert.ok(cursor.variables.required.includes(name), name);
  }
});

test("the Cursor plugin ships Claude Code's skills, without the section that connects Claude Code", () => {
  const skills = (dir) => readdirSync(join(root, dir, "skills")).sort();
  assert.deepEqual(skills(CURSOR), skills(CLAUDE));
  for (const name of skills(CLAUDE)) {
    const claude = skill(`${CLAUDE}/skills/${name}/SKILL.md`);
    const cursor = skill(`${CURSOR}/skills/${name}/SKILL.md`);
    assert.equal(cursor.name, claude.name);
    assert.ok(claude.description.startsWith(cursor.description.replace(/\.$/, "")), name);
    assert.equal(cursor.body, claude.body.replace(CONNECT, ""), name);
    assert.doesNotMatch(cursor.body, /claude mcp add/, name);
  }
});

// The plugin directory takes scripts, programs and other file types only from Verified organizations.
const ALLOWED = new Set([".md", ".markdown", ".txt", ".json", ".yaml", ".yml", ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico"]);
const NAMED = new Set(["LICENSE", ".gitignore"]);
const IMAGES = {
  ".png": (b) => b.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])),
  ".jpg": (b) => b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff,
  ".jpeg": (b) => b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff,
  ".gif": (b) => /^GIF8[79]a/.test(b.subarray(0, 6).toString("latin1")),
  ".webp": (b) => b.subarray(0, 4).toString("latin1") === "RIFF" && b.subarray(8, 12).toString("latin1") === "WEBP",
  ".ico": (b) => b.readUInt32BE(0) === 0x00000100,
};
const PACKAGE_MANAGER = new Set([
  "package.json",
  "package-lock.json",
  "npm-shrinkwrap.json",
  "pnpm-workspace.yaml",
  "pnpm-lock.yaml",
  "yarn.lock",
  ".yarnrc.yml",
  ".npmrc",
  "bun.lock",
  "deno.json",
  "composer.json",
  "requirements.txt",
]);
const utf8 = new TextDecoder("utf-8", { fatal: true });
const urls = (value, found = []) => {
  if (Array.isArray(value)) for (const item of value) urls(item, found);
  else if (value && typeof value === "object")
    for (const [key, item] of Object.entries(value)) {
      if ((key === "url" || key === "authServerMetadataUrl") && typeof item === "string") found.push(item);
      else urls(item, found);
    }
  return found;
};

test("every file of the Claude Code plugin is a type the plugin directory accepts from any organization", () => {
  for (const file of files(CLAUDE)) {
    const name = basename(file);
    const ext = extname(name).toLowerCase();
    const bytes = readFileSync(join(root, file));
    assert.ok(NAMED.has(name) || ALLOWED.has(ext), `${file} is not Markdown, text, JSON, YAML or a PNG, JPEG, GIF, WebP or ICO image`);
    assert.equal(PACKAGE_MANAGER.has(name), false, `${file} is a package manager's configuration`);
    assert.notEqual(bytes.subarray(0, 2).toString("latin1"), "#!", `${file} starts with #!`);
    if (IMAGES[ext]) {
      assert.ok(IMAGES[ext](bytes), `${file} is not the image its name says`);
      continue;
    }
    assert.doesNotThrow(() => utf8.decode(bytes), `${file} is not UTF-8`);
    assert.equal(bytes.includes(0), false, `${file} has a NUL byte`);
    if (ext === ".json")
      for (const url of urls(JSON.parse(bytes.toString("utf8")))) {
        assert.equal(/^\$\{user_config\.\w+\}$/.test(url) || !url.includes("${"), true, `${file}: ${url} has a variable`);
        assert.match(url, /^[\x21-\x7e]+$/, `${file}: ${url} is not plain ASCII`);
      }
  }
});
