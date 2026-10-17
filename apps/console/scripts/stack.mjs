import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repository = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const port = (name, fallback) => Number(process.env[name] ?? fallback);

export const STACKS = {
  dev: {
    project: process.env.KAESO_STACK_PROJECT ?? "kaeso-dev",
    publicUrl: process.env.KAESO_PUBLIC_URL ?? "http://localhost:3000",
    gatewayPort: port("KAESO_GATEWAY_PORT", 18080),
    idpPort: port("KAESO_IDP_PORT", 18091),
    mailPort: port("KAESO_MAIL_PORT", 58025),
    smtpPort: port("KAESO_MAIL_SMTP_PORT", 51025),
    clickhousePort: port("KAESO_CLICKHOUSE_PORT", 58123),
    postgresPort: port("KAESO_POSTGRES_PORT", 55432),
    valkeyPort: port("KAESO_VALKEY_PORT", 56379),
    profiles: ["console"],
  },
  e2e: {
    project: "kaeso-console-e2e",
    publicUrl: "http://localhost:3100",
    gatewayPort: 28080,
    idpPort: 28091,
    mailPort: 28025,
    smtpPort: 28026,
    clickhousePort: 28123,
    postgresPort: 28432,
    valkeyPort: 28379,
    profiles: ["console", "sso"],
    ssoPublicUrl: "http://localhost:3101",
    ssoGatewayPort: 28081,
  },
};

export function urls(stack) {
  return {
    gateway: `http://127.0.0.1:${stack.gatewayPort}`,
    ssoGateway: `http://127.0.0.1:${stack.ssoGatewayPort}`,
    mail: `http://127.0.0.1:${stack.mailPort}`,
    idp: `http://idp.localhost:${stack.idpPort}`,
    clickhouse: `http://127.0.0.1:${stack.clickhousePort}`,
  };
}

function compose(stack, args, options = {}) {
  return execFileSync(
    "docker",
    ["compose", "--file", "deploy/compose.dev.yaml", "--project-name", stack.project, ...stack.profiles.flatMap((p) => ["--profile", p]), ...args],
    {
      cwd: repository,
      encoding: "utf8",
      env: {
        ...process.env,
        KAESO_GATEWAY_IMAGE: process.env.KAESO_GATEWAY_IMAGE ?? `${stack.project}-gateway`,
        KAESO_PUBLIC_URL: stack.publicUrl,
        KAESO_GATEWAY_PORT: String(stack.gatewayPort),
        KAESO_IDP_PORT: String(stack.idpPort),
        KAESO_MAIL_PORT: String(stack.mailPort),
        KAESO_MAIL_SMTP_PORT: String(stack.smtpPort),
        KAESO_CLICKHOUSE_PORT: String(stack.clickhousePort),
        KAESO_POSTGRES_PORT: String(stack.postgresPort),
        KAESO_VALKEY_PORT: String(stack.valkeyPort),
        ...(stack.ssoPublicUrl ? { KAESO_SSO_PUBLIC_URL: stack.ssoPublicUrl, KAESO_SSO_GATEWAY_PORT: String(stack.ssoGatewayPort) } : {}),
      },
      ...options,
    },
  );
}

export function up(stack) {
  const build = process.env.KAESO_GATEWAY_IMAGE ? "--no-build" : "--build";
  compose(stack, ["up", "--detach", "--wait", "--quiet-pull", build], { stdio: "inherit" });
}

export function down(stack) {
  compose(stack, ["down", "--volumes", "--remove-orphans"], { stdio: "inherit" });
}

export function resetTwoFactor(stack, email) {
  compose(stack, ["exec", "--no-TTY", "gateway", "kaeso", "user", "reset-2fa", email], { stdio: ["ignore", "pipe", "inherit"] });
}

export function setupToken(stack, service = "gateway") {
  const printed = compose(stack, ["exec", "--no-TTY", service, "kaeso", "setup-link"], { stdio: ["ignore", "pipe", "inherit"] });
  return /#token=(\S+)/.exec(printed)?.[1] ?? null;
}

export function signInLinkToken(stack, email, service = "gateway") {
  const printed = compose(stack, ["exec", "--no-TTY", service, "kaeso", "user", "sign-in-link", email], { stdio: ["ignore", "pipe", "inherit"] });
  return /#token=(\S+)/.exec(printed)?.[1] ?? null;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command] = process.argv.slice(2);
  const stack = process.argv.includes("--e2e") ? STACKS.e2e : STACKS.dev;
  if (command === "up") {
    up(stack);
    const token = setupToken(stack);
    console.log(`Gateway on ${urls(stack).gateway}, mail on ${urls(stack).mail}`);
    if (token) console.log(`Set up Kaeso: ${stack.publicUrl}/setup#token=${token}`);
  } else if (command === "down") {
    down(stack);
  } else {
    console.error("usage: node scripts/stack.mjs up|down [--e2e]");
    process.exit(2);
  }
}
