---
title: 域名和 TLS
description: 在自己的域名下用证书提供 Kaeso，或把它放在已有的代理后面。
order: 5
---

一个实例只在一个域名下响应。Caddy 负责获取证书，把 `/v1` 和 `/platform` 转给网关，其他路径都转给控制台。

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
| `/v1/` 和 `/platform/` | 网关，端口 8080 |
| 其他所有路径 | 控制台，端口 3000 |

为 Kaeso 设置代理：

- 关闭 `/v1/` 的响应缓冲，让流式回复实时到达应用。
- 传递 `X-Forwarded-For`，并在 `.env` 中把 `KAESO_TRUSTED_PROXIES` 设为代理的确切地址。网关只信任来自该地址的这个请求头。Compose 包默认信任回环地址和 `edge` 网络，即 `EDGE_SUBNET`。服务器上调用上述端口的代理会从 `EDGE_SUBNET` 的第一个地址（默认为 `172.30.83.1`）到达网关。在不受信任的代理后面，日志和 [API 密钥限流](configuration.md#api-密钥限流)会把每个客户端都看作该代理。
- 如果用户通过 `https://` 加 `KAESO_DOMAIN` 以外的地址访问 Kaeso，请在 `.env` 中把 `KAESO_PUBLIC_URL` 设为该地址。
- 切勿转发指标端口 9100。

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

## 长时间的流

代理和负载均衡器会关闭一段时间内没有数据的连接，常见的是 60 或 100 秒。先推理再回答的模型可能沉默更久。所以当流处于打开状态、且已有 `KAESO_STREAMS_KEEPALIVE_INTERVAL`（默认 15 秒）没有写入任何内容时，网关会写入一个应用的 SDK 会忽略的保活信号：

- Chat Completions 和 Responses 使用 SSE 注释 `: keepalive`。
- Anthropic Messages 使用 `ping` 事件。

请让间隔明显短于链路上最短的空闲超时。服务商一响应，网关就发送回复的响应头，早于第一个事件。网关自己对流的限制是 `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` 和 `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT`（[服务商调用](configuration.md#服务商调用)）。

[请求流程](../concepts/request-flow.md)介绍了流中断或应用断开时会发生什么。

## 后续步骤

- [电子邮件](email.md)：连接 SMTP 服务器，让登录链接和邀请能送达。
- [健康状态和指标](monitoring.md)：检查实例的健康状态并抓取其指标。
