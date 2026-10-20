---
title: Domains and TLS
description: Serve Kaeso on your domain with a certificate, or behind a proxy you already run.
order: 5
---

An install answers on one domain. Caddy gets the certificate, sends `/v1` and `/platform` to the gateway, and sends every other path to the console.

## Set the domain

Set the domain in `.env` before the first start:

```bash title=".env"
KAESO_DOMAIN=kaeso.example.com
```

The public URL is `https://` and the domain. Setup links, emails and the console's code samples use it.

Because the console and the APIs share one origin, there is no cross-origin traffic to set up. Only web apps on other origins that call the model API from a browser need [`KAESO_CORS_ALLOWED_ORIGINS`](configuration.md#browser-apps).

## Get a certificate

Caddy gets a certificate from a public certificate authority at its first start and renews it on its own. Before you start Kaeso:

1. Point the domain's DNS record at the server.
2. Open ports 80 and 443 to the internet.

HTTPS requests can fail for a minute while Caddy gets the certificate. Run `docker compose logs caddy` to see when it has one, or why it can't. After you fix the DNS record or the firewall, run `docker compose restart caddy`.

Caddy keeps its certificates in the `caddy_data` volume. With `KAESO_DOMAIN=localhost`, it signs one with its own local authority instead ([trying it on your computer](installation.md#trying-it-on-your-computer)).

## Use your own reverse proxy

To run Kaeso behind a proxy or load balancer you already have, turn off Caddy and publish the gateway and the console on the loopback address:

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

Put this in `compose.override.yaml`, so an upgrade keeps it, and start Kaeso with `docker compose up -d --wait`. Without Caddy, nothing else waits for the gateway, so `--wait` is what makes Compose return only once it is healthy.

Then route these paths:

| Path | To |
| --- | --- |
| `/v1/` and `/platform/` | The gateway, port 8080 |
| Everything else | The console, port 3000 |

Set up the proxy for Kaeso:

- Turn off response buffering for `/v1/`, so streamed replies reach apps as they arrive.
- Pass `X-Forwarded-For`. The gateway believes it only from `KAESO_TRUSTED_PROXIES`. The Compose package sets it to the loopback and private networks, which only its own containers and the host's published ports reach. Elsewhere it is loopback only, so set it to your proxy's address. Behind an untrusted proxy, logs and [API key throttling](configuration.md#api-key-throttling) see every client as the proxy.
- If people reach Kaeso at another address than `https://` and `KAESO_DOMAIN`, set `KAESO_PUBLIC_URL` in `.env` to that address.
- Never route the metrics port, 9100.

### Connect a proxy that runs in Docker

When your proxy runs in Docker on the same server, connect it to Kaeso over a Docker network instead of published ports. Create a network for the proxy and Kaeso alone:

```bash
docker network create kaeso-proxy
```

Then join the gateway and the console to it, with aliases that no other stack uses:

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

Join your proxy's container to `kaeso-proxy` and route to `kaeso-gateway:8080` and `kaeso-console:3000`. The aliases matter because Compose makes every service name a DNS name on each network it joins. On a network that other stacks share, `gateway` could reach another stack's container.

## Long streams

Proxies and load balancers close connections that carry no bytes for a while, often after 60 or 100 seconds. A model that reasons before it answers can be silent for longer. So while a stream is open and nothing was written for `KAESO_STREAMS_KEEPALIVE_INTERVAL` (15 seconds by default), the gateway writes a keepalive that the app's SDK ignores:

- An SSE comment, `: keepalive`, for Chat Completions and Responses.
- A `ping` event for Anthropic Messages.

Keep the interval well below the shortest idle timeout on the way. The gateway sends the reply's headers as soon as the provider answers, before the first event. Its own limits for a stream are `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` and `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` ([provider calls](configuration.md#provider-calls)).

[Request flow](../concepts/request-flow.md) explains what happens when a stream breaks off or the app goes away.

## Next steps

- [Email](email.md): Connect an SMTP server so sign-in links and invitations arrive.
- [Health and metrics](monitoring.md): Check the install's health and scrape its metrics.
