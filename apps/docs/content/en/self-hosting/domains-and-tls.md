---
title: Domains and TLS
description: The domain an install answers on, its certificate, and running behind your own proxy.
order: 4
---

## One domain

An install answers on one domain. The console, the model API and the Platform API share it: Caddy sends `/v1/…` and `/platform/…` to the gateway and every other path to the console. Sharing the domain keeps the console's session cookie first-party, so there is no cross-origin traffic to configure.

`KAESO_DOMAIN` in `.env` sets the domain. The gateway's public URL, `KAESO_PUBLIC_URL`, follows from it as `https://<domain>`. It appears in setup links, invitation and sign-in emails, and in the console's code samples, and it must be `https`, except `http://localhost` for development.

## Certificates

Caddy gets a certificate for `KAESO_DOMAIN` from a public certificate authority when it first starts, and renews it by itself. For that to work:

- The domain's DNS record must point at the server before the first start.
- Ports 80 and 443 must be reachable from the internet.

`docker compose up -d --wait` doesn't wait for the certificate: Caddy reports healthy once it serves, and gets a new domain's certificate in the background, so the domain answers over HTTPS only once Caddy has it.

Caddy keeps its certificates in the `caddy_data` volume. If the certificate can't be issued, `docker compose logs caddy` says why. Fix the DNS record or the firewall, and restart Caddy with `docker compose restart caddy`.

With `KAESO_DOMAIN=localhost`, Caddy uses a certificate from its own local authority instead, which your browser asks you to accept once.

## Your own reverse proxy

To put Kaeso behind a proxy or load balancer you already run, turn off the `caddy` service and publish the gateway and the console on the loopback address in `compose.override.yaml`, so an upgrade keeps the change:

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

Start Kaeso with `docker compose up -d --wait`. Without Caddy, no service waits for the gateway and the console, so without `--wait` Compose returns before they answer; with it, Compose returns once both report healthy, and fails if one doesn't.

Then route:

| Path | To |
| --- | --- |
| `/v1/` and `/platform/` | the gateway, port 8080 |
| everything else | the console, port 3000 |

- Set `KAESO_PUBLIC_URL` in `.env` to the URL people use, such as `https://kaeso.example.com`, if it isn't `https://` and `KAESO_DOMAIN`.
- Turn off response buffering for `/v1/`, so streamed replies reach apps as they arrive.
- Pass `X-Forwarded-For`. The gateway believes it only from addresses in `KAESO_TRUSTED_PROXIES`, which are the private networks by default. With an untrusted proxy in front, every client looks like the proxy: logs show the proxy's address, and sign-in and [API key throttling](configuration.md#api-key-throttling) count every client together.
- Never route the metrics port, 9100.

### A proxy in Docker

When your proxy runs in Docker on the same server, connect it to the gateway and the console over a network instead of publishing their ports. Compose makes each service's name a DNS name on every network the service joins, so on a network other stacks share, `gateway` and `console` can resolve to another stack's container, and Kaeso's containers can answer requests meant for theirs. Create a network for the proxy and Kaeso alone, and give the two services names there that can't collide:

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

Join your proxy's container to `kaeso-proxy` and route to `kaeso-gateway:8080` and `kaeso-console:3000`. The services keep their names `gateway` and `console`, so commands such as `docker compose logs gateway` stay the same.

## Long streams

Proxies and load balancers close connections that carry no bytes for a while, many after 60 or 100 seconds. A model that thinks before it answers can be silent for longer, and an app in an OpenAI format that talks to a Claude model sees none of the thinking. So while a streamed reply is open and nothing was written to the app for `KAESO_STREAMS_KEEPALIVE_INTERVAL` (15 seconds), the gateway writes a keepalive the app's SDK ignores: an SSE comment (`: keepalive`) for Chat Completions and Responses, a `ping` event for Anthropic Messages.

- Keep the interval well below the shortest idle timeout on the way; `0s` turns keepalives off.
- The gateway sends the reply's headers as soon as the provider answers, before its first event.
- The gateway's own limits for a stream are `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` and `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT`; see [configuration](configuration.md#provider-calls).

What happens when a stream breaks off, or the app goes away, is described under [request flow](../concepts/request-flow.md).
