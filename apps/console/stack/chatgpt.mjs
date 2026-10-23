import { createHash, createSign, generateKeyPairSync, randomBytes, sign, timingSafeEqual } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { createServer as createHttpServer } from "node:http";
import { createServer as createHttpsServer } from "node:https";
import { dirname } from "node:path";
import { rootCertificates } from "node:tls";

const PORT = Number(process.env.PORT ?? 8080);
const TLS_PORT = Number(process.env.TLS_PORT ?? 443);
const CA_FILE = process.env.CA_FILE ?? "/run/chatgpt/ca.pem";
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

const base64url = (value) => Buffer.from(value).toString("base64url");

function same(a, b) {
  const left = Buffer.from(a);
  const right = Buffer.from(b);
  return left.length === right.length && timingSafeEqual(left, right);
}

const der = {
  length(n) {
    if (n < 0x80) return Buffer.from([n]);
    const bytes = [];
    for (let v = n; v > 0; v = Math.floor(v / 256)) bytes.unshift(v % 256);
    return Buffer.from([0x80 | bytes.length, ...bytes]);
  },
  tlv: (tag, body) => Buffer.concat([Buffer.from([tag]), der.length(body.length), body]),
  seq: (...items) => der.tlv(0x30, Buffer.concat(items)),
  set: (...items) => der.tlv(0x31, Buffer.concat(items)),
  int: (bytes) => der.tlv(0x02, bytes[0] & 0x80 ? Buffer.concat([Buffer.from([0]), bytes]) : bytes),
  oid(dotted) {
    const [a, b, ...rest] = dotted.split(".").map(Number);
    const out = [40 * a + b];
    for (const n of rest) {
      const septets = [n % 128];
      for (let v = Math.floor(n / 128); v > 0; v = Math.floor(v / 128)) septets.unshift(0x80 | (v % 128));
      out.push(...septets);
    }
    return der.tlv(0x06, Buffer.from(out));
  },
  utf8: (text) => der.tlv(0x0c, Buffer.from(text)),
  time: (date) => der.tlv(0x17, Buffer.from(`${date.toISOString().replace(/[-:T]/g, "").slice(2, 14)}Z`)),
  bool: (value) => der.tlv(0x01, Buffer.from([value ? 0xff : 0])),
  octets: (bytes) => der.tlv(0x04, bytes),
  bits: (bytes, unused = 0) => der.tlv(0x03, Buffer.concat([Buffer.from([unused]), bytes])),
  explicit: (n, body) => der.tlv(0xa0 + n, body),
  dnsName: (name) => der.tlv(0x82, Buffer.from(name)),
};

const ECDSA_SHA256 = "1.2.840.10045.4.3.2";
const name = (cn) => der.seq(der.set(der.seq(der.oid("2.5.4.3"), der.utf8(cn))));
const extension = (oid, critical, value) => der.seq(der.oid(oid), ...(critical ? [der.bool(true)] : []), der.octets(value));

function certificate({ subject, issuer, key, signer, extensions }) {
  const day = 24 * 3600 * 1000;
  const tbs = der.seq(
    der.explicit(0, der.int(Buffer.from([2]))),
    der.int(Buffer.concat([Buffer.from([1]), randomBytes(8)])),
    der.seq(der.oid(ECDSA_SHA256)),
    name(issuer),
    der.seq(der.time(new Date(Date.now() - day)), der.time(new Date(Date.now() + 30 * day))),
    name(subject),
    key.export({ type: "spki", format: "der" }),
    der.explicit(3, der.seq(...extensions)),
  );
  return der.seq(tbs, der.seq(der.oid(ECDSA_SHA256)), der.bits(sign("sha256", tbs, signer)));
}

const pem = (bytes) => `-----BEGIN CERTIFICATE-----\n${bytes.toString("base64").replace(/.{64}/g, "$&\n")}\n-----END CERTIFICATE-----\n`;

function tls() {
  const ca = generateKeyPairSync("ec", { namedCurve: "P-256" });
  const leaf = generateKeyPairSync("ec", { namedCurve: "P-256" });
  const authority = "Kaeso test authority";
  const caCert = certificate({
    subject: authority,
    issuer: authority,
    key: ca.publicKey,
    signer: ca.privateKey,
    extensions: [extension("2.5.29.19", true, der.seq(der.bool(true))), extension("2.5.29.15", true, der.bits(Buffer.from([0x06]), 1))],
  });
  const leafCert = certificate({
    subject: "auth.openai.com",
    issuer: authority,
    key: leaf.publicKey,
    signer: ca.privateKey,
    extensions: [
      extension("2.5.29.15", true, der.bits(Buffer.from([0x80]), 7)),
      extension("2.5.29.37", false, der.seq(der.oid("1.3.6.1.5.5.7.3.1"))),
      extension("2.5.29.17", false, der.seq(der.dnsName("auth.openai.com"), der.dnsName("api.openai.com"))),
    ],
  });
  mkdirSync(dirname(CA_FILE), { recursive: true });
  writeFileSync(CA_FILE, [pem(caCert), ...rootCertificates].join("\n"));
  return { key: leaf.privateKey.export({ type: "pkcs8", format: "pem" }), cert: pem(leafCert) };
}

function jwt(claims) {
  const header = base64url(JSON.stringify({ alg: "RS256", typ: "JWT", kid: KEY_ID }));
  const body = base64url(JSON.stringify(claims));
  const signature = createSign("RSA-SHA256").update(`${header}.${body}`).sign(privateKey).toString("base64url");
  return `${header}.${body}.${signature}`;
}

function send(res, status, body, headers = {}) {
  const json = typeof body !== "string";
  res.writeHead(status, { "content-type": json ? "application/json" : "text/plain; charset=utf-8", "cache-control": "no-store", ...headers });
  res.end(json ? JSON.stringify(body) : body);
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

async function readForm(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  return new URLSearchParams(Buffer.concat(chunks).toString("utf8"));
}

async function handle(req, res) {
  const url = new URL(req.url ?? "/", ISSUER);
  if (url.pathname === "/healthz") return send(res, 200, { status: "ok" });
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

const serve = (req, res) => {
  handle(req, res).catch((error) => {
    console.error(error);
    if (!res.headersSent) send(res, 500, { error: "server_error" });
    else res.end();
  });
};

createHttpsServer(tls(), serve).listen(TLS_PORT, () => console.log(`mock ChatGPT sign-in on https://auth.openai.com:${TLS_PORT}`));
createHttpServer(serve).listen(PORT, () => console.log(`mock ChatGPT sign-in for tests on http://127.0.0.1:${PORT}`));
