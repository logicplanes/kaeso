---
title: 域名和 TLS
description: 安装所用的域名、证书，以及在自己的代理之后运行。
order: 5
---

## 一个域名

一个安装只在一个域名上提供服务：Caddy 把 `/v1/…` 和 `/platform/…` 转发给网关，其他所有路径转发给控制台。控制台和 API 同源，因此控制台的会话 Cookie 始终是第一方 Cookie，无需配置任何跨源访问。在其他源上从浏览器调用模型 API 的 Web 应用需要配置 [`KAESO_CORS_ALLOWED_ORIGINS`](configuration.md#浏览器应用)。

`.env` 中的 `KAESO_DOMAIN` 设置域名，公开 URL `KAESO_PUBLIC_URL` 随之为 `https://<domain>`。设置链接、邮件和控制台中的代码示例都使用它。

## 证书

Caddy 首次启动时会从公共证书颁发机构为 `KAESO_DOMAIN` 申请证书，并自行续期。为此，在首次启动之前：

- 域名的 DNS 记录必须指向该服务器；
- 端口 80 和 443 必须能从互联网访问。

`docker compose up -d --wait` 不会等待证书：Caddy 一开始提供服务就报告健康，并在后台申请新域名的证书，因此要等 Caddy 拿到证书后，该域名才能通过 HTTPS 应答。

证书保存在 `caddy_data` 卷中。无法签发证书时，`docker compose logs caddy` 会显示证书颁发机构的答复；修正 DNS 记录或防火墙后，运行 `docker compose restart caddy`。

使用 `KAESO_DOMAIN=localhost` 时，Caddy 改用其本地证书颁发机构签发的证书；参见[在自己的电脑上试用](installation.md#在自己的电脑上试用)。

## 自己的反向代理

要在已有的代理或负载均衡器之后运行 Kaeso，关闭 `caddy` 服务，并把网关和控制台发布在回环地址上。请写在 `compose.override.yaml` 中，这样升级时会保留这些修改：

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

使用 `docker compose up -d --wait` 启动 Kaeso。没有 Caddy 时，没有别的服务等待网关和控制台；正是 `--wait` 让 Compose 等到两者都健康后才返回。

然后按如下方式转发：

| 路径 | 目标 |
| --- | --- |
| `/v1/` 和 `/platform/` | 网关，端口 8080 |
| 其他所有路径 | 控制台，端口 3000 |

- 如果人们访问 Kaeso 的地址不是 `https://` 加 `KAESO_DOMAIN`，在 `.env` 中把 `KAESO_PUBLIC_URL` 设为该地址，例如 `https://kaeso.example.com`。
- 为 `/v1/` 关闭响应缓冲，让流式回复一到达就传给应用。
- 传递 `X-Forwarded-For`。网关只信任来自 `KAESO_TRUSTED_PROXIES` 中网段的该请求头，默认是各私有地址段。位于不受信任的代理之后时，所有客户端看起来都是该代理：日志显示代理的地址，登录限流和 [API 密钥限流](configuration.md#api-密钥限流)把所有客户端算作一个。
- 永远不要转发指标端口 9100。

### 在 Docker 中运行的代理

如果自己的代理在同一台服务器上的 Docker 中运行，请通过 Docker 网络而不是发布端口把它连接到网关和控制台。为代理和 Kaeso 单独使用一个网络：Compose 会把每个服务名变成它所加入的每个网络中的 DNS 名称，在与其他栈共用的网络里，`gateway` 和 `console` 可能解析到其他栈的容器，Kaeso 的容器也可能响应发给其他栈的请求。创建该网络，并在其中为这两个服务指定不会冲突的别名：

```bash
docker network create kaeso-proxy
```

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

把代理容器加入 `kaeso-proxy`，并转发到 `kaeso-gateway:8080` 和 `kaeso-console:3000`。在 Compose 项目内，服务仍保留原名，因此 `docker compose logs gateway` 等命令不变。

## 长时间的流

代理和负载均衡器会关闭一段时间内没有传输字节的连接，通常在 60 或 100 秒之后。先推理再回答的模型可能沉默更久，而以 OpenAI 格式调用 Claude 模型的应用收不到任何推理内容。因此，在流式回复打开期间，如果 `KAESO_STREAMS_KEEPALIVE_INTERVAL`（默认 15 秒）内没有向应用写入任何内容，网关会写入一个应用 SDK 会忽略的保活消息：对 Chat Completions 和 Responses 是一条 SSE 注释（`: keepalive`），对 Anthropic Messages 是一个 `ping` 事件。

- 让间隔明显短于路径上最短的空闲超时；`0s` 关闭保活。
- 服务商一响应，网关就发送回复的响应头，早于第一个事件。
- 网关自身对流的限制是 `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` 和 `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT`；参见[服务商调用](configuration.md#服务商调用)。

流中断或应用离开时会发生什么，见[请求流程](../concepts/request-flow.md)。
