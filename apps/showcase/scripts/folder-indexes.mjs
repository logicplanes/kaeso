import { copyFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

function copyFolderIndexes(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) copyFolderIndexes(path);
    else if (name.endsWith(".html") && name !== "index.html") {
      const folder = join(dir, name.slice(0, -".html".length));
      if (existsSync(folder) && statSync(folder).isDirectory() && !existsSync(join(folder, "index.html"))) copyFileSync(path, join(folder, "index.html"));
    }
  }
}

export function folderIndexes() {
  return {
    name: "kaeso-folder-indexes",
    hooks: {
      "astro:build:done": ({ dir }) => copyFolderIndexes(fileURLToPath(dir)),
    },
  };
}
