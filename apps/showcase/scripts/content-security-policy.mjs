import { fileURLToPath } from "node:url";
import { writeSitePolicy } from "../../../scripts/csp.mjs";

export function contentSecurityPolicy() {
  return {
    name: "kaeso-content-security-policy",
    hooks: {
      "astro:build:done": ({ dir, logger }) => logger.info(writeSitePolicy(fileURLToPath(dir), { hashes: true, styleAttributes: true })),
    },
  };
}
