export const base64url = (value) => Buffer.from(value).toString("base64url");

export function send(res, status, body, headers = {}) {
  const json = typeof body !== "string";
  res.writeHead(status, { "content-type": json ? "application/json" : "text/plain; charset=utf-8", "cache-control": "no-store", ...headers });
  res.end(json ? JSON.stringify(body) : body);
}

export async function readForm(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  return new URLSearchParams(Buffer.concat(chunks).toString("utf8"));
}
