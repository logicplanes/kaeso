---
title: 域名和 TLS
description: 安装所用的域名、证书，以及在自己的代理之后运行。
order: 4
---

## 一个域名

一个安装只在一个域名上提供服务。控制台、模型 API 和 Platform API 共用这个域名：Caddy 将 `/v1/…` 和 `/platform/…` 发送给网关，其他所有路径发送给控制台。共用域名使控制台的会话 Cookie 保持为第一方 Cookie，因此无需配置任何跨源访问。

`.env` 中的 `KAESO_DOMAIN` 设置域名。网关的公开 URL `KAESO_PUBLIC_URL` 由它推导为 `https://<domain>`。该 URL 会出现在设置链接、邀请和登录邮件以及控制台的代码示例中，必须是 `https`，开发时的 `http://localhost` 除外。

## 证书

Caddy 在首次启动时从公共证书颁发机构为 `KAESO_DOMAIN` 获取证书，并自动续期。为此：

- 首次启动之前，域名的 DNS 记录必须已指向该服务器。
- 80 和 443 端口必须可以从互联网访问。

Caddy 将证书保存在 `caddy_data` 卷中。如果无法签发证书，`docker compose logs caddy` 会说明原因。修正 DNS 记录或防火墙，然后用 `docker compose restart caddy` 重启 Caddy。

使用 `KAESO_DOMAIN=localhost` 时，Caddy 改用其本地证书颁发机构签发的证书，浏览器会要求确认接受一次。

## 自己的反向代理

要将 Kaeso 放在已有的代理或负载均衡器之后，在 `compose.override.yaml` 中关闭 `caddy` 服务，并将网关和控制台发布到回环地址，这样升级时会保留这一修改：

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

然后按如下方式路由：

| 路径 | 目标 |
| --- | --- |
| `/v1/` 和 `/platform/` | 网关，8080 端口 |
| 其他所有路径 | 控制台，3000 端口 |

- 如果访问所用的 URL 不是 `https://` 加 `KAESO_DOMAIN`，在 `.env` 中将 `KAESO_PUBLIC_URL` 设为该 URL，例如 `https://kaeso.example.com`。
- 为 `/v1/` 关闭响应缓冲，使流式回复一到达就传给应用。
- 传递 `X-Forwarded-For`。网关只信任来自 `KAESO_TRUSTED_PROXIES` 中地址的该请求头，默认是各私有网络。如果前面是不受信任的代理，所有客户端看起来都是该代理：日志显示代理的地址，登录限流和 [API 密钥限流](configuration.md#api-密钥限流)会将所有客户端合并计数。
- 切勿路由指标端口 9100。

## 长时间的流

代理和负载均衡器会关闭一段时间内没有传输任何字节的连接，很多在 60 或 100 秒后关闭。先思考再回答的模型可能静默更长时间，而以 OpenAI 格式与 Claude 模型通信的应用看不到任何思考内容。因此，当流式回复处于打开状态、且在 `KAESO_STREAMS_KEEPALIVE_INTERVAL`（15 秒）内没有向应用写入任何内容时，网关会写入一条应用的 SDK 会忽略的保活消息：Chat Completions 和 Responses 为 SSE 注释（`: keepalive`），Anthropic Messages 为 `ping` 事件。

- 将间隔设得远低于途经各处中最短的空闲超时；`0s` 关闭保活。
- 服务商一响应，网关就会发送回复的响应头，早于第一个事件。
- 网关自身对流的限制是 `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` 和 `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT`；参见[配置](configuration.md#服务商调用)。

流中断或应用断开时会发生什么，见[请求流程](../concepts/request-flow.md)。
