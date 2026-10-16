import { randomBytes } from "node:crypto";
import { existsSync, writeFileSync } from "node:fs";

const file = process.argv[2] ?? "/run/kaeso/master_keys";
if (!existsSync(file)) writeFileSync(file, `k${randomBytes(5).toString("hex")}:${randomBytes(32).toString("base64")}`, { mode: 0o644 });
