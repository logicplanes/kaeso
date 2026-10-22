import { createHash, createSign, generateKeyPairSync, randomBytes, timingSafeEqual } from "node:crypto";
import { createServer } from "node:http";

const PORT = Number(process.env.PORT ?? 8091);
const ISSUER = process.env.ISSUER ?? `http://localhost:${PORT}`;
const CLIENT_ID = process.env.CLIENT_ID ?? "kaeso-console";
const CLIENT_SECRET = process.env.CLIENT_SECRET ?? "idp-client-secret";
const KEY_ID = "mock";

const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
const grants = new Map();

const base64url = (value) => Buffer.from(value).toString("base64url");
const escape = (value) => String(value).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

function same(a, b) {
  const left = Buffer.from(a);
  const right = Buffer.from(b);
  return left.length === right.length && timingSafeEqual(left, right);
}

function idToken(grant) {
  const issuedAt = Math.floor(Date.now() / 1000);
  const header = base64url(JSON.stringify({ alg: "RS256", typ: "JWT", kid: KEY_ID }));
  const claims = base64url(
    JSON.stringify({
      iss: ISSUER,
      sub: `sub-${createHash("sha256").update(grant.email).digest("hex").slice(0, 24)}`,
      aud: CLIENT_ID,
      iat: issuedAt,
      exp: issuedAt + 300,
      nonce: grant.nonce,
      auth_time: grant.authTime,
      email: grant.email,
      email_verified: true,
      name: grant.name,
    }),
  );
  const signature = createSign("RSA-SHA256").update(`${header}.${claims}`).sign(privateKey).toString("base64url");
  return `${header}.${claims}.${signature}`;
}

function send(res, status, body, headers = {}) {
  const json = typeof body !== "string";
  res.writeHead(status, { "content-type": json ? "application/json" : "text/html; charset=utf-8", "cache-control": "no-store", ...headers });
  res.end(json ? JSON.stringify(body) : body);
}

function discovery() {
  return {
    issuer: ISSUER,
    authorization_endpoint: `${ISSUER}/authorize`,
    token_endpoint: `${ISSUER}/token`,
    jwks_uri: `${ISSUER}/jwks`,
    response_types_supported: ["code"],
    subject_types_supported: ["public"],
    id_token_signing_alg_values_supported: ["RS256"],
    scopes_supported: ["openid", "email", "profile"],
    token_endpoint_auth_methods_supported: ["client_secret_basic", "client_secret_post"],
    claims_supported: ["sub", "email", "email_verified", "name"],
    code_challenge_methods_supported: ["S256"],
  };
}

function signInPage(params) {
  const hidden = [...params].map(([name, value]) => `<input type="hidden" name="${escape(name)}" value="${escape(value)}">`).join("");
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Mock identity provider</title></head><body><main><h1>Mock identity provider</h1><form method="post" action="/authorize">${hidden}<label>Email <input name="login_email" type="email" required></label><label>Name <input name="login_name" required></label><button type="submit">Sign in</button></form></main></body></html>`;
}

function authorize(form) {
  if (form.get("client_id") !== CLIENT_ID) return { error: "unknown client_id" };
  if (form.get("response_type") !== "code") return { error: "response_type must be code" };
  if (form.get("code_challenge_method") !== "S256" || !form.get("code_challenge")) return { error: "PKCE with S256 is required" };
  let redirect;
  try {
    redirect = new URL(form.get("redirect_uri") ?? "");
  } catch {
    return { error: "redirect_uri is missing" };
  }
  const code = base64url(randomBytes(24));
  grants.set(code, {
    nonce: form.get("nonce") ?? "",
    challenge: form.get("code_challenge"),
    redirect: redirect.href,
    email: form.get("login_email") ?? "",
    name: form.get("login_name") ?? "",
    authTime: Math.floor(Date.now() / 1000),
  });
  redirect.searchParams.set("code", code);
  redirect.searchParams.set("state", form.get("state") ?? "");
  return { location: redirect.href };
}

function clientOf(req, form) {
  const basic = /^Basic (.+)$/.exec(req.headers.authorization ?? "")?.[1];
  if (basic) {
    const [id, secret] = Buffer.from(basic, "base64").toString("utf8").split(":").map(decodeURIComponent);
    return { id, secret };
  }
  return { id: form.get("client_id"), secret: form.get("client_secret") };
}

function token(req, form) {
  const client = clientOf(req, form);
  if (client.id !== CLIENT_ID || !same(client.secret ?? "", CLIENT_SECRET)) return [401, { error: "invalid_client" }];
  if (form.get("grant_type") !== "authorization_code") return [400, { error: "unsupported_grant_type" }];
  const code = form.get("code") ?? "";
  const grant = grants.get(code);
  grants.delete(code);
  if (!grant || grant.redirect !== form.get("redirect_uri")) return [400, { error: "invalid_grant" }];
  const challenge = createHash("sha256")
    .update(form.get("code_verifier") ?? "")
    .digest("base64url");
  if (!same(challenge, grant.challenge)) return [400, { error: "invalid_grant", error_description: "PKCE verification failed" }];
  return [200, { access_token: base64url(randomBytes(24)), token_type: "Bearer", expires_in: 300, id_token: idToken(grant) }];
}

async function readForm(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  return new URLSearchParams(Buffer.concat(chunks).toString("utf8"));
}

async function handle(req, res) {
  const url = new URL(req.url ?? "/", ISSUER);
  if (url.pathname === "/healthz") return send(res, 200, { status: "ok" });
  if (url.pathname === "/.well-known/openid-configuration") return send(res, 200, discovery());
  if (url.pathname === "/jwks") return send(res, 200, { keys: [{ ...publicKey.export({ format: "jwk" }), kid: KEY_ID, alg: "RS256", use: "sig" }] });
  if (url.pathname === "/authorize" && req.method === "GET") return send(res, 200, signInPage(url.searchParams));
  if (url.pathname === "/authorize" && req.method === "POST") {
    const result = authorize(await readForm(req));
    return result.location ? send(res, 303, "", { location: result.location }) : send(res, 400, { error: "invalid_request", error_description: result.error });
  }
  if (url.pathname === "/token" && req.method === "POST") {
    const [status, body] = token(req, await readForm(req));
    return send(res, status, body);
  }
  return send(res, 404, { error: "not_found" });
}

createServer((req, res) => {
  handle(req, res).catch((error) => {
    console.error(error);
    if (!res.headersSent) send(res, 500, { error: "server_error" });
    else res.end();
  });
}).listen(PORT, () => console.log(`mock identity provider ${ISSUER}`));
