---
title: Domains and TLS
description: The domain an install answers on, its certificate, and running behind your own proxy.
order: 4
---

## One domain

An install answers on one domain: Caddy sends `/v1/…` and `/platform/…` to the gateway and every other path to the console. Because the console and the APIs share an origin, the console's session cookie stays first-party and there is no cross-origin traffic to configure. Web apps on other origins that call the model API from a browser need [`KAESO_CORS_ALLOWED_ORIGINS`](configuration.md#browser-apps).

`KAESO_DOMAIN` in `.env` sets the domain, and the public URL, `KAESO_PUBLIC_URL`, follows as `https://<domain>`. Setup links, emails and the console's code samples use it.

## Certificates

Caddy gets a certificate for `KAESO_DOMAIN` from a public certificate authority at its first start and renews it on its own. For that, before the first start:

- the domain's DNS record must point at the server;
- ports 80 and 443 must be reachable from the internet.

`docker compose up -d --wait` doesn't wait for the certificate: Caddy reports healthy once it serves and gets a new domain's certificate in the background, so the domain answers over HTTPS only once Caddy has it.

Certificates are kept in the `caddy_data` volume. When one can't be issued, `docker compose logs caddy` shows the certificate authority's answer; fix the DNS record or the firewall, then run `docker compose restart caddy`.

With `KAESO_DOMAIN=localhost`, Caddy uses a certificate from its own local authority instead; see [trying it on your computer](installation.md#trying-it-on-your-computer).

## Your own reverse proxy

To run Kaeso behind a proxy or load balancer you already have, turn off the `caddy` service and publish the gateway and the console on the loopback address, in `compose.override.yaml` so an upgrade keeps the change:

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

Start Kaeso with `docker compose up -d --wait`. Without Caddy, nothing else waits for the gateway and the console, so `--wait` is what makes Compose return only once both are healthy.

Then route:

| Path | To |
| --- | --- |
| `/v1/` and `/platform/` | The gateway, port 8080 |
| Everything else | The console, port 3000 |

- If people reach Kaeso at another address than `https://` and `KAESO_DOMAIN`, set `KAESO_PUBLIC_URL` in `.env` to it, such as `https://kaeso.example.com`.
- Turn off response buffering for `/v1/`, so streamed replies reach apps as they arrive.
- Pass `X-Forwarded-For`. The gateway believes it only from the networks in `KAESO_TRUSTED_PROXIES`, the private ranges by default. Behind an untrusted proxy every client looks like the proxy: logs show its address, and sign-in and [API key throttling](configuration.md#api-key-throttling) count all clients as one.
- Never route the metrics port, 9100.

### A proxy in Docker

When your proxy runs in Docker on the same server, connect it to the gateway and the console over a Docker network instead of published ports. Use a network for the proxy and Kaeso alone: Compose makes each service's name a DNS name on every network it joins, so on a network other stacks share, `gateway` and `console` could resolve to another stack's containers, or Kaeso's to theirs. Create the network, then give the two services aliases there that can't collide:

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

Join your proxy's container to `kaeso-proxy` and route to `kaeso-gateway:8080` and `kaeso-console:3000`. Inside the Compose project the services keep their names, so commands such as `docker compose logs gateway` stay the same.

## Long streams

Proxies and load balancers close connections that carry no bytes for a while, often after 60 or 100 seconds. A model that reasons before it answers can be silent for longer, and an app in an OpenAI format that calls a Claude model receives none of that reasoning. So while a streamed reply is open and nothing was written to the app for `KAESO_STREAMS_KEEPALIVE_INTERVAL` (default 15 seconds), the gateway writes a keepalive the app's SDK ignores: an SSE comment (`: keepalive`) for Chat Completions and Responses, a `ping` event for Anthropic Messages.

- Keep the interval well below the shortest idle timeout on the way; `0s` turns keepalives off.
- The gateway sends the reply's headers as soon as the provider answers, before its first event.
- The gateway's own limits for a stream are `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` and `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT`; see [provider calls](configuration.md#provider-calls).

[Request flow](../concepts/request-flow.md) describes what happens when a stream breaks off or the app goes away.
