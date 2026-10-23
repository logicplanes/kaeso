import { createHash, createSign, generateKeyPairSync, randomBytes, timingSafeEqual } from "node:crypto";
import { base64url, readForm, send } from "./http.mjs";

const ISSUER = "https://auth.openai.com";
const RESOURCE = "https://api.openai.com/v1";
const PLAN_SCOPE = "chatgpt.tokens.use.direct";
const FIRST_CLIENT = "dynamic_agent_client";
const CLIENT_ID = "oaiapp_mock_console";
const ACCOUNT = { sub: "sub-owner", email: "owner@example.com" };
const KEY_ID = "mock";

const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
const grants = new Map();
const sessions = new Map();
const revoked = [];

function same(a, b) {
  const left = Buffer.from(a);
  const right = Buffer.from(b);
  return left.length === right.length && timingSafeEqual(left, right);
}

function jwt(claims) {
  const header = base64url(JSON.stringify({ alg: "RS256", typ: "JWT", kid: KEY_ID }));
  const body = base64url(JSON.stringify(claims));
  const signature = createSign("RSA-SHA256").update(`${header}.${body}`).sign(privateKey).toString("base64url");
  return `${header}.${body}.${signature}`;
}

function authorize(query) {
  const requested = query.get("client_id");
  if (requested !== FIRST_CLIENT && requested !== CLIENT_ID) return { error: "unknown client_id" };
  if (query.get("response_type") !== "code" || query.get("code_challenge_method") !== "S256") return { error: "PKCE with S256 is required" };
  if (query.get("resource") !== RESOURCE || !query.get("scope")?.split(" ").includes(PLAN_SCOPE)) return { error: "plan usage was not requested" };
  if (!query.get("ext_agent_host_id")?.startsWith("urn:uuid:")) return { error: "ext_agent_host_id is required" };
  let redirect;
  try {
    redirect = new URL(query.get("redirect_uri") ?? "");
  } catch {
    return { error: "redirect_uri is missing" };
  }
  if (redirect.hostname !== "127.0.0.1" || redirect.pathname !== "/auth/callback") return { error: "redirect_uri must be the loopback callback" };
  const code = base64url(randomBytes(24));
  grants.set(code, { nonce: query.get("nonce") ?? "", challenge: query.get("code_challenge"), redirect: redirect.href });
  redirect.searchParams.set("code", code);
  redirect.searchParams.set("scope", query.get("scope") ?? "");
  redirect.searchParams.set("state", query.get("state") ?? "");
  if (requested === FIRST_CLIENT) redirect.searchParams.set("client_id", CLIENT_ID);
  return { location: redirect.href };
}

function tokens(nonce) {
  const now = Math.floor(Date.now() / 1000);
  const access = jwt({
    iss: ISSUER,
    aud: RESOURCE,
    sub: ACCOUNT.sub,
    client_id: CLIENT_ID,
    scope: PLAN_SCOPE,
    iat: now,
    exp: now + 3600,
    jti: base64url(randomBytes(9)),
  });
  const refresh = `rt_${base64url(randomBytes(24))}`;
  sessions.set(refresh, access);
  const body = {
    access_token: access,
    refresh_token: refresh,
    token_type: "Bearer",
    expires_in: 3600,
    scope: `${PLAN_SCOPE} email offline_access openid profile resource.invoke`,
    earliest_refresh_at: now,
  };
  if (nonce !== undefined) body.id_token = jwt({ iss: ISSUER, aud: CLIENT_ID, sub: ACCOUNT.sub, email: ACCOUNT.email, nonce, iat: now, exp: now + 3600 });
  return body;
}

function token(form) {
  if (form.get("resource") !== RESOURCE) return [400, { error: "invalid_target" }];
  if (form.get("client_id") !== CLIENT_ID) return [401, { error: "invalid_client" }];
  if (form.get("grant_type") === "refresh_token") {
    const refresh = form.get("refresh_token") ?? "";
    if (!sessions.has(refresh)) return [400, { error: "refresh_token_reused" }];
    sessions.delete(refresh);
    return [200, tokens(undefined)];
  }
  if (form.get("grant_type") !== "authorization_code") return [400, { error: "unsupported_grant_type" }];
  const code = form.get("code") ?? "";
  const grant = grants.get(code);
  grants.delete(code);
  if (!grant || grant.redirect !== form.get("redirect_uri")) return [400, { error: "invalid_grant" }];
  const challenge = createHash("sha256")
    .update(form.get("code_verifier") ?? "")
    .digest("base64url");
  if (!same(challenge, grant.challenge)) return [400, { error: "invalid_grant" }];
  return [200, tokens(grant.nonce)];
}

export async function chatgpt(req, res, url) {
  if (url.pathname === "/__mock/revoked") return send(res, 200, { revoked: revoked.length });
  if (url.pathname === "/.well-known/jwks.json")
    return send(res, 200, { keys: [{ ...publicKey.export({ format: "jwk" }), kid: KEY_ID, alg: "RS256", use: "sig" }] });
  if (url.pathname === "/api/accounts/authorize" && req.method === "GET") {
    const result = authorize(url.searchParams);
    return result.location ? send(res, 302, "", { location: result.location }) : send(res, 400, { error: "invalid_request", error_description: result.error });
  }
  if (url.pathname === "/api/accounts/oauth/token" && req.method === "POST") {
    const [status, body] = token(await readForm(req));
    return send(res, status, body);
  }
  if (url.pathname === "/api/accounts/oauth/revoke" && req.method === "POST") {
    const form = await readForm(req);
    sessions.delete(form.get("token") ?? "");
    revoked.push(form.get("client_id"));
    return send(res, 200, "");
  }
  return send(res, 404, { error: "not_found" });
}
