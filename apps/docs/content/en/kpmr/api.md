---
title: KPMR API
description: The public endpoints of KPMR, how to fetch the catalogue and keep it current, and how to verify its signature.
order: 3
---

KPMR's catalogue and its smaller documents are open to everyone at `https://kpmr.kaeso.app`, without a key or an account. This request fetches the catalogue of two providers and prints the input and output prices of `claude-haiku-4-5`:

```bash tab="curl"
curl -s "https://kpmr.kaeso.app/v1/catalogue?providers=anthropic,openai" \
  -H "Accept: application/vnd.kaeso.kpmr+json; version=1" \
  --compressed \
  | jq -r '.providers.anthropic.models["claude-haiku-4-5"].pricing
    | "\(.input) \(.output)"'
```

```python tab="Python"
import json
import urllib.request

request = urllib.request.Request(
    "https://kpmr.kaeso.app/v1/catalogue?providers=anthropic,openai",
    headers={"Accept": "application/vnd.kaeso.kpmr+json; version=1"},
)
with urllib.request.urlopen(request) as response:
    catalogue = json.load(response)

models = catalogue["providers"]["anthropic"]["models"]
pricing = models["claude-haiku-4-5"]["pricing"]
print(pricing["input"], pricing["output"])
```

```ts tab="TypeScript"
const response = await fetch(
  "https://kpmr.kaeso.app/v1/catalogue?providers=anthropic,openai",
  { headers: { Accept: "application/vnd.kaeso.kpmr+json; version=1" } },
);
const catalogue = await response.json();

const { pricing } = catalogue.providers.anthropic.models["claude-haiku-4-5"];
console.log(pricing.input, pricing.output);
```

It prints the prices in US dollars per million tokens:

```text
1 5
```

The curl example needs `jq`. The Python needs only the standard library, and the TypeScript runs in a browser or in Node.js 18 or later. [Catalogue format](catalogue.md) explains every field of the answer.

## Endpoints

| Method and path | Answers |
| --- | --- |
| `GET /v1/catalogue` | The signed catalogue. `?providers=` names the providers you need |
| `GET /v1/meta.json` | The revision, the format, counts, the providers Kaeso calls in `kaeso_providers`, and the signing keys |
| `GET /v1/providers.json` | Every provider with its number of models, and whether Kaeso calls it in `kaeso` |
| `GET /v1/providers/<provider>.json` | One provider: its details, its lines with their history, its registry entry and a summary of each model |
| `GET /v1/models/<provider>/<model>.json` | One model: its catalogue entry, the lines that point at it, its registry entry with sources, announced prices in `upcoming`, and the pricing rules that apply |
| `GET /v1/recent.json` | The newest models of the providers Kaeso calls. `?limit=` takes 1 to 50, and 12 without it |
| `GET /v1/deprecations.json` | Every upcoming shutdown across providers, soonest first |
| `GET /v1/search.json` | The site's search index: the providers Kaeso calls and their models |
| `GET /v1/keys.json` | The public keys catalogues are signed with |
| `GET /healthz` | `status`, whether the catalogue is `ready`, and its `revision` |

Only the catalogue is signed. The JSON documents allow any origin, so a web page can read them, and every endpoint answers `HEAD` and `OPTIONS` too. A model id keeps its `/`, as in `/v1/models/fal/alibaba/wan-3.0/text-to-video.json`; encode other characters as in any address. An unknown provider or model answers `404` with an `error`.

## The catalogue

### Query

| Parameter | Meaning |
| --- | --- |
| `providers` | Up to 64 provider ids, separated by commas. An id KPMR doesn't know is left out of `providers` rather than refused, and `provider_ids` always lists every id. Without it, you get every provider, more than 10 MB before compression |

### Request headers

| Header | Meaning |
| --- | --- |
| `Accept` | `application/vnd.kaeso.kpmr+json; version=1`, the format version you read, so a later format can't surprise your code. `application/json` and no `Accept` get format 1 too |
| `If-None-Match` | The `ETag` of the catalogue you have. KPMR answers `304` with no body while it hasn't changed |
| `Accept-Encoding` | `br` or `gzip` for a compressed answer |
| `User-Agent` | Your app's name, so KPMR can tell its readers apart |

### Response headers

| Header | Meaning |
| --- | --- |
| `ETag` | Send it back in `If-None-Match` next time |
| `KPMR-Revision` | The catalogue's `revision` |
| `KPMR-Signature` | `keyid=<key id>, ed25519=<signature>`, the Ed25519 signature of the body ([verify the signature](#verify-the-signature)) |
| `Cache-Control` | `public, max-age=60`; a cache may serve a stale copy while it asks again |

### Status codes

| Status | Meaning |
| --- | --- |
| `200` | The catalogue, signed, and compressed when you accept it |
| `304` | Unchanged since the `ETag` you sent: keep what you have |
| `400` | `providers` is empty, names more than 64 providers or holds something that isn't a provider id. `error` says which |
| `406` | No format version you accept, such as `version=2`. `supported` lists the ones KPMR serves |
| `503` | KPMR is still starting. Try again after `Retry-After` |

An error answers with a JSON body:

```json wrap
{ "error": "KPMR serves the catalogue as application/vnd.kaeso.kpmr+json in format 1.", "supported": [1] }
```

## Keep it current

Keep the catalogue's `ETag` and its `refresh_seconds`. Ask again once that many seconds have passed, with the `ETag` in `If-None-Match`, and keep the catalogue you have while KPMR answers `304` or can't be reached. A Kaeso gateway does exactly this ([keeping model data current](sync.md)).

## Verify the signature

KPMR signs the exact bytes of the body before compression, which is what you have once `gzip` or Brotli is undone. Check the signature before you trust a copy from a mirror or a cache, and pin the public key in your code rather than fetching it next to the catalogue. `/v1/keys.json` lists the keys with their ids.

```python tab="Python"
import base64
import urllib.request

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

PUBLIC_KEY = "EQI7HVO5opUSzyPkqSLOOkOPFoYH+QywDUYDHaQUnd8="

request = urllib.request.Request(
    "https://kpmr.kaeso.app/v1/catalogue?providers=anthropic",
    headers={"Accept": "application/vnd.kaeso.kpmr+json; version=1"},
)
with urllib.request.urlopen(request) as response:
    body = response.read()
    header = response.headers["KPMR-Signature"]

fields = dict(part.strip().split("=", 1) for part in header.split(","))
key = Ed25519PublicKey.from_public_bytes(base64.b64decode(PUBLIC_KEY))
key.verify(base64.b64decode(fields["ed25519"]), body)
print("Signed with", fields["keyid"])
```

```ts tab="TypeScript"
import { createPublicKey, verify } from "node:crypto";

const PUBLIC_KEY = "EQI7HVO5opUSzyPkqSLOOkOPFoYH+QywDUYDHaQUnd8=";

const key = createPublicKey({
  key: {
    kty: "OKP",
    crv: "Ed25519",
    x: Buffer.from(PUBLIC_KEY, "base64").toString("base64url"),
  },
  format: "jwk",
});

const response = await fetch(
  "https://kpmr.kaeso.app/v1/catalogue?providers=anthropic",
  { headers: { Accept: "application/vnd.kaeso.kpmr+json; version=1" } },
);
const body = Buffer.from(await response.arrayBuffer());
const fields = Object.fromEntries(
  response.headers
    .get("KPMR-Signature")
    .split(",")
    .map((part) => part.trim().split(/=(.*)/s)),
);

if (!verify(null, body, key, Buffer.from(fields.ed25519, "base64"))) {
  throw new Error("The catalogue's signature doesn't match.");
}
console.log("Signed with", fields.keyid);
```

The Python needs the `cryptography` package (`pip install cryptography`) and raises an error when the signature doesn't match. The TypeScript needs Node.js 18 or later.

## Deprecations

`/v1/deprecations.json` lists every model with an announced shutdown that hasn't come yet, across providers, soonest first. `kaeso` says whether Kaeso calls the provider:

```json wrap
[
  {
    "provider": "anthropic",
    "provider_name": "Anthropic",
    "kaeso": true,
    "id": "claude-sonnet-4-5-20250929",
    "name": "Claude Sonnet 4.5",
    "status": "deprecated",
    "lifecycle": {
      "announced_on": "2026-09-30",
      "deprecated_on": "2026-09-30",
      "shutdown_on": "2026-11-30",
      "replacement": "claude-sonnet-5-5",
      "source": "https://platform.claude.com/docs/en/about-claude/model-deprecations"
    }
  }
]
```

## Terms

The catalogue is free to use, with or without Kaeso. Cache it and send `If-None-Match` rather than fetching it often. The data is MIT licensed: keep the notices in [KPMR's licenses](https://kpmr.kaeso.app/licenses) with any copy you pass on. Provider and model names belong to their owners.

## Next steps

- [Catalogue format](catalogue.md): Look up every field of the catalogue.
- [Keeping model data current](sync.md): See how a Kaeso gateway fetches and trusts the catalogue.
- [Retirements and model lines](lifecycle-and-lines.md): See how the lifecycle dates and lines change.
