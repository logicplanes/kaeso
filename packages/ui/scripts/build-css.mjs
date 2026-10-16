import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
mkdirSync(join(dist, "fonts"), { recursive: true });

cpSync(join(root, "src/styles/fonts"), join(dist, "fonts"), { recursive: true });
for (const f of ["fonts.css", "kaeso.css"]) cpSync(join(root, "src/styles", f), join(dist, f));

execFileSync(join(root, "node_modules/.bin/tailwindcss"), ["-i", "src/styles/styles.css", "-o", "dist/styles.css", "--minify"], {
  cwd: root,
  stdio: "inherit",
});

const tw = readFileSync(join(root, "src/styles/tailwind.css"), "utf8").replace(
  /@source "\.\.\/components";\n@source "\.\.\/blocks";\n@source "\.\.\/theme";/,
  '@source "./components";\n@source "./blocks";\n@source "./chunks";\n@source "./index.js";',
);
writeFileSync(join(dist, "tailwind.css"), tw);
console.log("@logicplanes/kaeso-ui: wrote styles.css, tailwind.css and fonts");
