// Kaeso on Railway: a public edge, a private gateway and console, Railway Postgres and Redis.
// Docs: https://docs.kaeso.app/self-hosting/platforms/railway
import { defineRailway, image, postgres, project, redis, service } from "railway/iac";

const GATEWAY_IMAGE = "ghcr.io/logicplanes/kaeso-gateway:0.6.0-rc.4";
const CONSOLE_IMAGE = "ghcr.io/logicplanes/kaeso-console:0.6.0-rc.4";

// Railway's private network is IPv6 only, so the gateway listens on every address family and
// trusts the edge from the unique local range.
const TRUSTED_PROXIES = "fc00::/7,10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,127.0.0.0/8,::1/128";

export default defineRailway((ctx) => {
  const db = postgres("kaeso-postgres");
  const cache = redis("kaeso-valkey");

  const gateway = service("kaeso-gateway", {
    source: image(GATEWAY_IMAGE, { autoUpdates: { type: "disabled" } }),
    healthcheck: "/healthz",
    healthcheckTimeout: 300,
    env: {
      KAESO_LISTEN: "[::]:8080",
      KAESO_METRICS_LISTEN: "127.0.0.1:9100",
      KAESO_PUBLIC_URL: ctx.shared.KAESO_PUBLIC_URL,
      KAESO_MASTER_KEYS: ctx.shared.KAESO_MASTER_KEYS,
      KAESO_SETUP_EMAIL: ctx.shared.KAESO_SETUP_EMAIL,
      KAESO_CLICKHOUSE_URL: ctx.shared.KAESO_CLICKHOUSE_URL,
      KAESO_SMTP_HOST: ctx.shared.KAESO_SMTP_HOST,
      KAESO_SMTP_PORT: ctx.shared.KAESO_SMTP_PORT,
      KAESO_SMTP_USER: ctx.shared.KAESO_SMTP_USER,
      KAESO_SMTP_PASSWORD: ctx.shared.KAESO_SMTP_PASSWORD,
      KAESO_SMTP_FROM: ctx.shared.KAESO_SMTP_FROM,
      KAESO_POSTGRES_URL: db.env.DATABASE_URL,
      KAESO_VALKEY_URL: cache.env.REDIS_URL,
      KAESO_TRUSTED_PROXIES: TRUSTED_PROXIES,
      KAESO_LOG_FORMAT: "json",
    },
  });

  const console = service("kaeso-console", {
    source: image(CONSOLE_IMAGE, { autoUpdates: { type: "disabled" } }),
    healthcheck: "/healthz",
  });

  const edge = service("kaeso-edge", {
    source: image(CONSOLE_IMAGE, { autoUpdates: { type: "disabled" } }),
    start: "kaeso-edge",
    healthcheck: "/kaeso-edge/healthz",
    env: {
      PORT: "8080",
      KAESO_GATEWAY_UPSTREAM: "${{kaeso-gateway.RAILWAY_PRIVATE_DOMAIN}}:8080",
      KAESO_CONSOLE_UPSTREAM: "${{kaeso-console.RAILWAY_PRIVATE_DOMAIN}}:3000",
    },
  });

  return project("kaeso", {
    resources: [db, cache, gateway, console, edge],
  });
});
