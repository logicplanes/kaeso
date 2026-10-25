import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const json = (path) => JSON.parse(readFileSync(join(root, path), "utf8"));
const version = json("package.json").version;
const plugins = readdirSync(join(root, "plugins"));
const references = (text, pattern) => [...text.matchAll(pattern)].map((m) => m[1]);
const KEBAB = /^[a-z0-9]+(-[a-z0-9]+)*$/;

test("each marketplace lists every plugin under its folder's name", () => {
  for (const [file, manifest] of [
    [".claude-plugin/marketplace.json", ".claude-plugin/plugin.json"],
    [".cursor-plugin/marketplace.json", ".cursor-plugin/plugin.json"],
  ]) {
    const market = json(file);
    assert.match(market.name, KEBAB, file);
    assert.ok(market.owner?.name, file);
    assert.deepEqual(market.plugins.map((p) => p.name).sort(), [...plugins].sort(), file);
    for (const entry of market.plugins) {
      const folder = entry.source.replace(/^\.\//, "");
      assert.equal(folder, `plugins/${entry.name}`, file);
      assert.equal(json(join(folder, manifest)).name, entry.name, `${file}: ${entry.name}`);
    }
  }
});

for (const name of plugins) {
  const dir = `plugins/${name}`;

  test(`${name} ships the release's version, its license, a README and its pictures`, () => {
    const claude = json(`${dir}/.claude-plugin/plugin.json`);
    const cursor = json(`${dir}/.cursor-plugin/plugin.json`);
    assert.equal(claude.version, version);
    assert.equal(cursor.version, version);
    assert.equal(claude.license, json("package.json").license);
    assert.ok(existsSync(join(root, dir, "LICENSE")));
    const words = readFileSync(join(root, dir, "README.md"), "utf8")
      .split(/\s+/)
      .filter(Boolean);
    assert.ok(words.length >= 40, "the directories ask for a README of 40 words or more");
    assert.match(claude.icon, /^\.\/[\w/-]+\.png$/);
    assert.deepEqual([...readFileSync(join(root, dir, claude.icon)).subarray(1, 4)], [...Buffer.from("PNG")]);
    assert.match(cursor.logo, /^[\w/-]+\.svg$/);
    assert.match(readFileSync(join(root, dir, cursor.logo), "utf8"), /^<svg [^>]*viewBox="0 0 512 512"/);
    for (const url of [claude.homepage, claude.documentationUrl, claude.supportUrl, claude.privacyPolicyUrl, cursor.homepage]) assert.match(url, /^https:\/\//);
  });

  test(`${name} asks Claude Code for the instance's address and reaches its MCP server there`, () => {
    const claude = json(`${dir}/.claude-plugin/plugin.json`);
    const servers = json(`${dir}/.mcp.json`).mcpServers;
    for (const server of Object.values(servers)) {
      assert.equal(server.type, "http");
      assert.match(server.url, /^\$\{user_config\.\w+\}\/mcp$/);
    }
    const used = references(readFileSync(join(root, dir, ".mcp.json"), "utf8"), /\$\{user_config\.(\w+)\}/g);
    for (const key of used) {
      assert.match(key, /^[A-Za-z_]\w*$/);
      assert.equal(claude.userConfig[key]?.type, "string", key);
      assert.equal(claude.userConfig[key]?.required, true, key);
      assert.ok(claude.userConfig[key]?.title && claude.userConfig[key]?.description, key);
    }
  });

  test(`${name} asks Cursor for the instance's address and reaches its MCP server there`, () => {
    const cursor = json(`${dir}/.cursor-plugin/plugin.json`);
    const text = readFileSync(join(root, dir, "mcp.json"), "utf8");
    for (const server of Object.values(JSON.parse(text).mcpServers)) assert.match(server.url, /^\$\{\w+\}\/mcp$/);
    for (const name of references(text, /\$\{(\w+)\}/g)) {
      assert.equal(cursor.variables.properties[name]?.type, "string", name);
      assert.ok(cursor.variables.required.includes(name), name);
    }
  });

  test(`${name}'s skills say when to use them`, () => {
    for (const skill of readdirSync(join(root, dir, "skills"))) {
      const text = readFileSync(join(root, dir, "skills", skill, "SKILL.md"), "utf8");
      const front = /^---\n([\s\S]*?)\n---\n/.exec(text)?.[1] ?? "";
      assert.match(front, new RegExp(`^name: ${skill}$`, "m"));
      assert.match(front, /^description: .{40,}$/m);
    }
  });
}
