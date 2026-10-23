import { generateKeyPairSync, randomBytes, sign } from "node:crypto";
import { base64url, readForm, send } from "./http.mjs";

const ISSUER = "https://auth.x.ai";
const CLIENT_ID = "b1a00492-073a-47ea-816f-4c329264a828";
const SCOPES = "openid profile email offline_access grok-cli:access api:access";
const PAGE = "https://accounts.x.ai/oauth2/device";
const DEVICE_GRANT = "urn:ietf:params:oauth:grant-type:device_code";
const ACCOUNT = { sub: "sub-grok-owner", email: "owner@example.com" };
const KEY_ID = "mock-es256";
const INTERVAL = 2;
const LIFETIME = 1800;
const MODELS = [{ id: "grok-4.3", object: "model", created: 1790000000, owned_by: "xai" }];

const { privateKey, publicKey } = generateKeyPairSync("ec", { namedCurve: "P-256" });
const devices = new Map();
const sessions = new Map();
const revoked = [];

function jwt(claims) {
  const header = base64url(JSON.stringify({ alg: "ES256", typ: "JWT", kid: KEY_ID }));
  const body = base64url(JSON.stringify(claims));
  const signature = sign("sha256", Buffer.from(`${header}.${body}`), { key: privateKey, dsaEncoding: "ieee-p1363" }).toString("base64url");
  return `${header}.${body}.${signature}`;
}

function isForm(req) {
  return (req.headers["content-type"] ?? "").startsWith("application/x-www-form-urlencoded");
}

function deviceCode(form) {
  if (form.get("client_id") !== CLIENT_ID) return [400, { error: "invalid_client" }];
  if (!form.get("scope")?.split(" ").includes("api:access")) return [400, { error: "invalid_scope" }];
  const code = base64url(randomBytes(32));
  const user = randomBytes(4).toString("hex").toUpperCase();
  const userCode = `${user.slice(0, 4)}-${user.slice(4)}`;
  devices.set(code, { userCode, approved: false });
  return [
    200,
    {
      device_code: code,
      user_code: userCode,
      verification_uri: PAGE,
      verification_uri_complete: `${PAGE}?user_code=${userCode}`,
      expires_in: LIFETIME,
      interval: INTERVAL,
    },
  ];
}

function granted(withIdToken) {
  const now = Math.floor(Date.now() / 1000);
  const access = jwt({ iss: ISSUER, aud: "https://api.x.ai", sub: ACCOUNT.sub, iat: now, exp: now + LIFETIME, jti: base64url(randomBytes(9)) });
  const refresh = `xai-refresh-${base64url(randomBytes(24))}`;
  sessions.set(refresh, access);
  const body = { access_token: access, refresh_token: refresh, token_type: "Bearer", expires_in: LIFETIME, scope: SCOPES };
  if (withIdToken)
    body.id_token = jwt({ iss: ISSUER, aud: CLIENT_ID, sub: ACCOUNT.sub, email: ACCOUNT.email, email_verified: true, iat: now, exp: now + 3600 });
  return body;
}

function token(form) {
  if (form.get("client_id") !== CLIENT_ID) return [400, { error: "invalid_client" }];
  if (form.get("grant_type") === "refresh_token") {
    const refresh = form.get("refresh_token") ?? "";
    if (!sessions.has(refresh)) return [400, { error: "invalid_grant" }];
    sessions.delete(refresh);
    return [200, granted(false)];
  }
  if (form.get("grant_type") !== DEVICE_GRANT) return [400, { error: "unsupported_grant_type" }];
  const code = form.get("device_code") ?? "";
  const device = devices.get(code);
  if (!device) return [400, { error: "invalid_grant" }];
  if (!device.approved) return [400, { error: "authorization_pending" }];
  devices.delete(code);
  return [200, granted(true)];
}

function bearer(req) {
  const value = req.headers.authorization ?? "";
  return value.startsWith("Bearer ") ? value.slice(7) : "";
}

function signedIn(req) {
  return [...sessions.values()].includes(bearer(req));
}

function api(req, res, url) {
  if (!signedIn(req))
    return send(res, 400, { code: "invalid-argument", error: "Incorrect API key provided. You can obtain an API key from https://console.x.ai." });
  if (url.pathname === "/v1/models") return send(res, 200, { object: "list", data: MODELS });
  const model = MODELS.find((m) => url.pathname === `/v1/models/${m.id}`);
  if (model) return send(res, 200, model);
  return send(res, 404, { code: "not-found", error: "No such route." });
}

export async function grok(req, res, url) {
  if (url.pathname === "/__mock/grok/revoked") return send(res, 200, { revoked: revoked.length });
  if (url.pathname === "/.well-known/jwks.json")
    return send(res, 200, { keys: [{ ...publicKey.export({ format: "jwk" }), kid: KEY_ID, alg: "ES256", use: "sig" }] });
  if (url.pathname === "/oauth2/device" && req.method === "GET") {
    const device = [...devices.values()].find((d) => d.userCode === url.searchParams.get("user_code"));
    if (!device) return send(res, 404, "No such code.");
    device.approved = true;
    return send(res, 200, "Approved.");
  }
  if (["/oauth2/device/code", "/oauth2/token", "/oauth2/revoke"].includes(url.pathname) && req.method === "POST") {
    if (!isForm(req)) return send(res, 415, "Form requests must have `Content-Type: application/x-www-form-urlencoded`");
    const form = await readForm(req);
    if (url.pathname === "/oauth2/revoke") {
      sessions.delete(form.get("token") ?? "");
      revoked.push(form.get("client_id"));
      return send(res, 200, "");
    }
    const [status, body] = url.pathname === "/oauth2/token" ? token(form) : deviceCode(form);
    return send(res, status, body);
  }
  if (url.pathname === "/oauth2/userinfo") {
    if (!signedIn(req)) return send(res, 401, { error: "invalid_token" });
    return send(res, 200, { sub: ACCOUNT.sub, email: ACCOUNT.email, email_verified: true });
  }
  if (url.pathname.startsWith("/v1/")) return api(req, res, url);
  return send(res, 404, { error: "not_found" });
}
