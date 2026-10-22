import { mergeConfig } from "vite";
import base from "../vite.config";

export default mergeConfig(base, {
  server: {
    forwardConsole: { unhandledErrors: true, logLevels: ["error"] },
    headers: { "Content-Security-Policy": "require-trusted-types-for 'script'" },
  },
});
