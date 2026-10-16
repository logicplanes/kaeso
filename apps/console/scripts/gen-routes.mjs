import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Generator, getConfig } from "@tanstack/router-generator";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
await new Generator({ config: getConfig({ target: "react" }, root), root }).run();
console.log("gen:routes: wrote src/routeTree.gen.ts");
