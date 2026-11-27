---
title: 域名和 TLS
description: 在自己的域名下用证书提供 Kaeso，或把它放在已有的代理后面。
order: 5
---

一个实例只在一个域名下响应。Caddy 负责获取证书，把各个 API 以及 MCP 服务器及其登录转给网关，其他路径都转给控制台。

## 设置域名

在首次启动前，在 `.env` 中设置域名：

```bash title=".env"
KAESO_DOMAIN=kaeso.example.com
```

公开 URL 为 `https://` 加该域名。设置链接、邮件和控制台中的代码示例都使用它。

由于控制台和 API 共用同一个源，不需要配置任何跨域访问。只有在其他源上、从浏览器调用模型 API 的 Web 应用才需要 [`KAESO_CORS_ALLOWED_ORIGINS`](configuration.md#浏览器应用)。

## 获取证书

Caddy 首次启动时从公共证书颁发机构获取证书，并自动续期。启动 Kaeso 之前：

1. 把域名的 DNS 记录指向服务器。
2. 向互联网开放端口 80 和 443。

Caddy 获取证书期间，HTTPS 请求可能会失败一分钟左右。运行 `docker compose logs caddy` 可以看到它何时拿到证书，或者为什么拿不到。修正 DNS 记录或防火墙后，运行 `docker compose restart caddy`。

Caddy 把证书保存在 `caddy_data` 卷中。设置 `KAESO_DOMAIN=localhost` 时，它改用自己的本地证书颁发机构签发证书（[在自己的电脑上试用](installation.md#在自己的电脑上试用)）。

## 使用自己的反向代理

要在已有的代理或负载均衡器后面运行 Kaeso，请关闭 Caddy，并在回环地址上发布网关和控制台：

```yaml
services:
  caddy:
    profiles: [disabled]
  gateway:
    ports:
      - "127.0.0.1:8080:8080"
  console:
    ports:
      - "127.0.0.1:3000:3000"
```

把它写进 `compose.override.yaml`，升级时就会保留，然后用 `docker compose up -d --wait` 启动 Kaeso。没有 Caddy 时，再没有别的服务等待网关，所以要靠 `--wait` 让 Compose 等网关健康后才返回。

然后按以下规则转发路径：

| 路径 | 转发到 |
| --- | --- |
| `/v1/`、`/v1beta/`、`/v2/`、`/platform/`、`/mcp`、`/mcp/`、`/oauth/`、`/ingest/` 和 `/.well-known/oauth-` | 网关，端口 8080 |
| 其他所有路径 | 控制台，端口 3000 |

智能体通过 `/oauth/` 和 `/.well-known/oauth-` 登录，然后调用 `/mcp`（[MCP 服务器](../api/mcp.md)）；MCP 客户端在 `/mcp/<项目 ID>` 调用项目的门户。如果代理把这些路径转给控制台，任何智能体都无法连接。

为 Kaeso 设置代理：

- 关闭 `/v1/` 和 `/v1beta/` 的响应缓冲，让流式回复实时到达应用。
- 在 `/v1/` 上放行 WebSocket 升级，让 ElevenLabs 应用能打开[实时会话](../api/elevenlabs.md#实时会话)。
- 放行不超过网关最大[请求体上限](configuration.md#请求体与响应体)的请求体（默认 64 MiB），例如通过 nginx 的 `client_max_body_size`。更大的请求体由网关以应用的格式返回 413。网关在应答前会把每个请求体读完（最多 1 GiB），即使是它不读内容就拒绝的请求也是如此，因此流式转发请求体的代理（例如 Traefik）会把 413 或拒绝原样传回。
- 传递 `X-Forwarded-For`，并在 `.env` 中把 `KAESO_TRUSTED_PROXIES` 设为代理的确切地址。网关只信任来自该地址的这个请求头。Compose 包默认信任回环地址和 `edge` 网络，即 `EDGE_SUBNET`。服务器上调用上述端口的代理会从 `EDGE_SUBNET` 的第一个地址（默认为 `172.30.83.1`）到达网关。在不受信任的代理后面，日志和 [API 密钥限流](configuration.md#api-密钥限流)会把每个客户端都看作该代理。
- 如果用户通过 `https://` 加 `KAESO_DOMAIN` 以外的地址访问 Kaeso，请在 `.env` 中把 `KAESO_PUBLIC_URL` 设为该地址。
- 切勿转发指标端口 9100。

如果想保留 Caddy，让它改为向你的代理提供普通 HTTP，请把一个端口作为域名、把代理的地址传给安装程序：`sh -s -- --domain :80 --public-url https://kaeso.example.com`。Caddy 随后在 80 端口上不使用证书地应答，安装程序会把 `KAESO_PUBLIC_URL` 和该地址下的发件人写入 `.env`。不加 `--public-url` 时，地址为 `http://localhost` 加上该端口。

### 连接在 Docker 中运行的代理

如果代理在同一台服务器的 Docker 中运行，请通过 Docker 网络而不是发布的端口把它连接到 Kaeso。为代理和 Kaeso 单独创建一个网络：

```bash
docker network create --subnet 172.30.84.0/24 kaeso-proxy
```

然后把网关和控制台加入该网络，并使用其他栈不会用到的别名：

```yaml
services:
  caddy:
    profiles: [disabled]
  gateway:
    networks:
      kaeso-proxy:
        aliases: [kaeso-gateway]
  console:
    networks:
      kaeso-proxy:
        aliases: [kaeso-console]

networks:
  kaeso-proxy:
    external: true
```

把代理的容器以固定地址（例如 `172.30.84.2`）加入 `kaeso-proxy`，在 `.env` 中设置 `KAESO_TRUSTED_PROXIES=172.30.84.2`，并转发到 `kaeso-gateway:8080` 和 `kaeso-console:3000`。之所以需要别名，是因为 Compose 会在服务加入的每个网络中把服务名注册为 DNS 名称。在其他栈也在用的网络上，`gateway` 可能会指向另一个栈的容器。

## 搜索引擎

在您允许之前，实例不对搜索引擎和 AI 爬虫开放：它的任何页面都不会出现在搜索结果中，也不会被提供用于训练模型或生成助手的回答。Kaeso 保管着您的服务商密钥和组织数据，因此默认情况下，它的地址不应被他人找到。

如果希望别人能通过搜索找到您的登录页面，请在 `.env` 中设置 `KAESO_SEARCH_ENGINES=true`，然后运行 `docker compose up -d --wait`。Compose 会把这个变量传给网关和控制台，两者都会读取它；Helm chart 和各平台模板同样只在一处设置它（[配置](configuration.md#搜索引擎)）。实例设置的**常规**页面会显示它的当前值。

| 爬虫得到的内容 | 关闭（默认） | 开启 |
| --- | --- | --- |
| `/robots.txt` | 禁止所有路径 | 允许 `/sign-in`、`/sign-up` 以及显示它们所需的文件，禁止其他所有路径 |
| `X-Robots-Tag: noindex, nofollow` | 每个响应都带有 | 除这些页面和文件外，每个响应都带有 |
| `<meta name="robots" content="noindex, nofollow">` | 每个页面都带有 | 除这两个页面外，每个页面都带有 |

即使开启，以下内容仍然不会被收录：

- 带查询参数的 `/sign-in` 和 `/sign-up`，因为登录页面的 `next` 会写出访问者之前所在的页面。
- `/`，它会把未登录的访问者带到登录页面，以及其后控制台的所有页面。
- 共享链接，它们只面向收到链接的人。
- API、MCP 服务器及其登录。网关的每个响应都带有 `noindex`，在网关自己的地址上，它的 `robots.txt` 禁止所有路径。

`robots.txt` 没有点名任何爬虫，因此它的规则适用于每个读取它的爬虫，无论是搜索引擎还是 AI 爬虫，例如 Googlebot、OpenAI 的 GPTBot 和 OAI-SearchBot、Anthropic 的 ClaudeBot、PerplexityBot 以及 Common Crawl 的 CCBot。爬虫最多保留一天的副本，控制台允许它们保留一小时。

这些是守规矩的爬虫会遵守的请求，而不是访问控制：除登录页面外，所有内容本来就需要登录。遵守 `robots.txt` 的搜索引擎从不抓取被禁止的页面，因此也读不到该页面的 `noindex`。如果其他网站链接到您的实例，它仍可能列出光秃秃的地址，但没有标题，也没有其中的任何文字。助手因有人要求而打开链接时，可能会在这一次访问中忽略 `robots.txt`，它看到的与任何未登录访问者看到的相同。

## 长时间的流

代理和负载均衡器会关闭一段时间内没有数据的连接，常见的是 60 或 100 秒。先推理再回答的模型可能沉默更久。所以当流处于打开状态、且已有 `KAESO_STREAMS_KEEPALIVE_INTERVAL`（默认 15 秒）没有写入任何内容时，网关会写入一个应用的 SDK 会忽略的保活信号：

- Chat Completions 和 Responses 使用 SSE 注释 `: keepalive`。
- Anthropic Messages 使用 `ping` 事件。

请让间隔明显短于链路上最短的空闲超时。服务商一响应，网关就发送回复的响应头，早于第一个事件。网关自己对流的限制是 `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` 和 `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT`（[服务商调用](configuration.md#服务商调用)）。

[请求流程](../concepts/request-flow.md)介绍了流中断或应用断开时会发生什么。

## 下一步

- [电子邮件](email.md)：连接 SMTP 服务器，让登录链接和邀请能送达。
- [健康状态和指标](monitoring.md)：检查实例的健康状态并抓取其指标。
