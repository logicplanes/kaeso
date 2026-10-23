import { generateKeyPairSync, randomBytes, sign } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { createServer as createHttpServer } from "node:http";
import { createServer as createHttpsServer } from "node:https";
import { dirname } from "node:path";
import { rootCertificates } from "node:tls";
import { chatgpt } from "./chatgpt.mjs";
import { grok } from "./grok.mjs";
import { send } from "./http.mjs";

const PORT = Number(process.env.PORT ?? 8080);
const TLS_PORT = Number(process.env.TLS_PORT ?? 443);
const CA_FILE = process.env.CA_FILE ?? "/run/accounts/ca.pem";
const HOSTS = ["auth.openai.com", "api.openai.com", "auth.x.ai", "api.x.ai"];
const XAI = /^(auth|api)\.x\.ai(:\d+)?$/;

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
    subject: HOSTS[0],
    issuer: authority,
    key: leaf.publicKey,
    signer: ca.privateKey,
    extensions: [
      extension("2.5.29.15", true, der.bits(Buffer.from([0x80]), 7)),
      extension("2.5.29.37", false, der.seq(der.oid("1.3.6.1.5.5.7.3.1"))),
      extension("2.5.29.17", false, der.seq(...HOSTS.map(der.dnsName))),
    ],
  });
  mkdirSync(dirname(CA_FILE), { recursive: true });
  writeFileSync(CA_FILE, [pem(caCert), ...rootCertificates].join("\n"));
  return { key: leaf.privateKey.export({ type: "pkcs8", format: "pem" }), cert: pem(leafCert) };
}

function handle(req, res) {
  const url = new URL(req.url ?? "/", "http://127.0.0.1");
  if (url.pathname === "/healthz") return send(res, 200, { status: "ok" });
  const xai = XAI.test(req.headers.host ?? "") || url.pathname.startsWith("/oauth2/") || url.pathname.startsWith("/__mock/grok/");
  return xai ? grok(req, res, url) : chatgpt(req, res, url);
}

const serve = (req, res) => {
  Promise.resolve(handle(req, res)).catch((error) => {
    console.error(error);
    if (!res.headersSent) send(res, 500, { error: "server_error" });
    else res.end();
  });
};

createHttpsServer(tls(), serve).listen(TLS_PORT, () => console.log(`mock ChatGPT and Grok sign-in on https://${HOSTS.join(", https://")}:${TLS_PORT}`));
createHttpServer(serve).listen(PORT, () => console.log(`mock ChatGPT and Grok sign-in for tests on http://127.0.0.1:${PORT}`));
