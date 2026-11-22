---
title: KPMR API
description: KPMR 的公开端点、如何获取目录并保持其最新，以及如何验证其签名。
order: 3
---

KPMR 的目录及其较小的文档在 `https://kpmr.kaeso.app` 上向所有人开放，无需密钥或账户。下面的请求获取两个服务商的目录，并打印 `claude-haiku-4-5` 的输入和输出价格：

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

它打印以每百万 Token 美元计的价格：

```text
1 5
```

curl 示例需要 `jq`。Python 示例只需要标准库，TypeScript 可在浏览器或 Node.js 18 及以上版本中运行。[目录格式](catalogue.md)说明了应答中的每个字段。

## 端点

| 方法和路径 | 返回 |
| --- | --- |
| `GET /v1/catalogue` | 已签名的目录。`?providers=` 指定您需要的服务商 |
| `GET /v1/meta.json` | 修订版本、格式、计数、`kaeso_providers` 中 Kaeso 调用的服务商，以及签名密钥 |
| `GET /v1/providers.json` | 每个服务商及其模型数量，`kaeso` 表示 Kaeso 是否调用它 |
| `GET /v1/providers/<provider>.json` | 一个服务商：其详情、模型系列及历史、注册表条目和每个模型的摘要 |
| `GET /v1/models/<provider>/<model>.json` | 一个模型：其目录条目、指向它的模型系列、带来源的注册表条目、`upcoming` 中已宣布的价格，以及适用的定价规则 |
| `GET /v1/recent.json` | Kaeso 调用的服务商的最新模型。`?limit=` 取 1 到 50，不传时为 12 |
| `GET /v1/deprecations.json` | 所有服务商即将到来的停用，最近的排在前面 |
| `GET /v1/search.json` | 网站的搜索索引：Kaeso 调用的服务商及其模型 |
| `GET /v1/keys.json` | 用于为目录签名的公钥 |
| `GET /healthz` | `status`、目录是否就绪（`ready`）及其 `revision` |

只有目录带有签名。JSON 文档允许任何来源访问，因此网页可以读取它们，每个端点也响应 `HEAD` 和 `OPTIONS`。模型 ID 保留其中的 `/`，例如 `/v1/models/fal/alibaba/wan-3.0/text-to-video.json`；其他字符按普通地址编码。未知的服务商或模型会返回 `404` 和 `error`。

## 目录

### 查询参数

| 参数 | 含义 |
| --- | --- |
| `providers` | 最多 64 个服务商 ID，以逗号分隔。KPMR 不认识的 ID 会从 `providers` 中略去而不是被拒绝，`provider_ids` 始终列出每个 ID。不传时返回所有服务商，压缩前超过 10 MB |

### 请求标头

| 标头 | 含义 |
| --- | --- |
| `Accept` | `application/vnd.kaeso.kpmr+json; version=1`，即您读取的格式版本，以免后续格式让您的代码出错。`application/json` 或不带 `Accept` 时也会得到格式 1 |
| `If-None-Match` | 您已有目录的 `ETag`。目录没有变化时，KPMR 返回不带响应体的 `304` |
| `Accept-Encoding` | `br` 或 `gzip`，用于获取压缩的应答 |
| `User-Agent` | 您的应用名称，便于 KPMR 区分不同的读取方 |

### 响应标头

| 标头 | 含义 |
| --- | --- |
| `ETag` | 下次请求时放在 `If-None-Match` 中发回 |
| `KPMR-Revision` | 目录的 `revision` |
| `KPMR-Signature` | `keyid=<key id>, ed25519=<signature>`，响应体的 Ed25519 签名（[验证签名](#验证签名)） |
| `Cache-Control` | `public, max-age=60`；缓存在重新请求期间可以提供过期的副本 |

### 状态码

| 状态 | 含义 |
| --- | --- |
| `200` | 已签名的目录，您接受压缩时会压缩 |
| `304` | 自您发送的 `ETag` 以来没有变化：保留已有的目录 |
| `400` | `providers` 为空、超过 64 个服务商，或包含不是服务商 ID 的内容。`error` 会说明原因 |
| `406` | 没有您接受的格式版本，例如 `version=2`。`supported` 列出 KPMR 提供的版本 |
| `503` | KPMR 仍在启动。请在 `Retry-After` 之后重试 |

错误以 JSON 响应体返回：

```json wrap
{ "error": "KPMR serves the catalogue as application/vnd.kaeso.kpmr+json in format 1.", "supported": [1] }
```

## 保持目录最新

保存目录的 `ETag` 和 `refresh_seconds`。经过这么多秒后再次请求，并在 `If-None-Match` 中带上 `ETag`；KPMR 返回 `304` 或无法访问时，继续使用已有的目录。Kaeso 网关正是这样做的（[保持模型数据最新](sync.md)）。

## 验证签名

KPMR 为压缩前的响应体原始字节签名，也就是解除 `gzip` 或 Brotli 压缩后得到的内容。在信任来自镜像或缓存的副本之前，请先检查签名，并把公钥固定在代码中，而不是与目录一起获取。`/v1/keys.json` 列出各个密钥及其 ID。

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
const signature = response.headers.get("KPMR-Signature");
if (!signature) throw new Error("The catalogue isn't signed.");
const fields = Object.fromEntries(
  signature.split(",").map((part) => part.trim().split(/=(.*)/s)),
);

if (!verify(null, body, key, Buffer.from(fields.ed25519, "base64"))) {
  throw new Error("The catalogue's signature doesn't match.");
}
console.log("Signed with", fields.keyid);
```

Python 示例需要 `cryptography` 包（`pip install cryptography`），签名不匹配时会抛出错误。TypeScript 需要 Node.js 18 或更高版本。

## 弃用计划

`/v1/deprecations.json` 列出所有服务商中已宣布但尚未到来的停用模型，最近的排在前面。`kaeso` 表示 Kaeso 是否调用该服务商：

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

## 使用条款

无论是否使用 Kaeso，目录都可以免费使用。请缓存目录并发送 `If-None-Match`，而不要频繁获取。数据采用 MIT 许可证：传递任何副本时，请附上 [KPMR 许可证](https://kpmr.kaeso.app/zh-CN/licenses)中的声明。服务商和模型名称归其所有者所有。

## 下一步

- [目录格式](catalogue.md)：查阅目录的每个字段。
- [保持模型数据最新](sync.md)：了解 Kaeso 网关如何获取并信任目录。
- [停用和模型系列](lifecycle-and-lines.md)：了解生命周期日期和模型系列如何变化。
