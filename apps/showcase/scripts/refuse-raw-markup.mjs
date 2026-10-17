import { basename } from "node:path";

const CHANGELOG = "CHANGELOG.md";

const isSafeAddress = (url) => {
  const address = [...url].filter((character) => character.charCodeAt(0) > 32).join("");
  return /^(https?|mailto):/i.test(address) || !/^[^/?#]*:/.test(address);
};

const refusal = (node) => {
  if (node.type === "html") return "raw HTML is not allowed; write it in a code span or remove it";
  const linked = node.type === "link" || node.type === "image" || node.type === "definition";
  return linked && !isSafeAddress(node.url ?? "") ? "links may only use http, https or mailto" : null;
};

const check = (node, name) => {
  const reason = refusal(node);
  if (reason) throw new Error(`${name} line ${node.position?.start.line ?? "?"}: ${reason}`);
  for (const child of node.children ?? []) check(child, name);
};

export const fragmentSource = (markdown, line, definitions = "") =>
  `${"\n".repeat(Math.max(0, line - 1))}${markdown}${definitions ? `\n\n${definitions}` : ""}`;

export function refuseRawMarkup() {
  return (tree, file) => {
    const name = basename(file.path ?? "");
    if (name === CHANGELOG) check(tree, name);
  };
}
