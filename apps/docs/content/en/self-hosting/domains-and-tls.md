---
title: Domains and TLS
description: Serve Kaeso on your domain with a certificate, or behind a proxy you already run.
order: 5
---

An install answers on one domain. Caddy gets the certificate, sends the APIs and the MCP server with its sign-in to the gateway, and sends every other path to the console.

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
| `/v1/`, `/v1beta/`, `/v2/`, `/platform/`, `/mcp`, `/mcp/`, `/oauth/`, `/ingest/` and `/.well-known/oauth-` | The gateway, port 8080 |
| Everything else | The console, port 3000 |

Agents sign in through `/oauth/` and `/.well-known/oauth-` and then call `/mcp` ([MCP server](../api/mcp.md)); MCP clients call a project's portal at `/mcp/<project id>`. A proxy that sends these paths to the console stops every agent from connecting.

Set up the proxy for Kaeso:

- Turn off response buffering for `/v1/` and `/v1beta/`, so streamed replies reach apps as they arrive.
- Pass WebSocket upgrades on `/v1/`, so ElevenLabs apps can open [realtime sessions](../api/elevenlabs.md#realtime-sessions).
- Let request bodies through up to the gateway's largest [body limit](configuration.md#request-and-response-bodies), 64 MiB by default, such as with nginx's `client_max_body_size`. The gateway answers a larger body with 413 in the app's format. It reads every body to its end before it answers, up to 1 GiB, also one it refuses unread, so a proxy that streams it, such as Traefik, passes the 413 or the refusal on.
- Pass `X-Forwarded-For`, and set `KAESO_TRUSTED_PROXIES` in `.env` to your proxy's exact address. The gateway believes the header only from there. By default, the Compose package trusts loopback and the `edge` network, `EDGE_SUBNET`. A proxy on the server that calls the ports above reaches the gateway from the first address of `EDGE_SUBNET`, `172.30.83.1` by default. Behind an untrusted proxy, logs and [API key throttling](configuration.md#api-key-throttling) see every client as the proxy.
- If people reach Kaeso at another address than `https://` and `KAESO_DOMAIN`, set `KAESO_PUBLIC_URL` in `.env` to that address.
- Never route the metrics port, 9100.

To keep Caddy and let it serve plain HTTP to your proxy instead, give the installer a port as the domain and your proxy's address: `sh -s -- --domain :80 --public-url https://kaeso.example.com`. Caddy then answers on port 80 without a certificate, and the installer writes `KAESO_PUBLIC_URL` and a sender at that address to `.env`. Without `--public-url`, the address is `http://localhost` and the port.

### Connect a proxy that runs in Docker

When your proxy runs in Docker on the same server, connect it to Kaeso over a Docker network instead of published ports. Create a network for the proxy and Kaeso alone:

```bash
docker network create --subnet 172.30.84.0/24 kaeso-proxy
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

Join your proxy's container to `kaeso-proxy` at a fixed address, such as `172.30.84.2`, set `KAESO_TRUSTED_PROXIES=172.30.84.2` in `.env`, and route to `kaeso-gateway:8080` and `kaeso-console:3000`. The aliases matter because Compose makes every service name a DNS name on each network it joins. On a network that other stacks share, `gateway` could reach another stack's container.

## Search engines

An install asks search engines and AI crawlers to stay out until you let them in: they index none of its pages, and none is offered for training models or for an assistant's answers. Kaeso holds your provider keys and your organizations' data, so by default its address is nobody else's to find.

Set `KAESO_SEARCH_ENGINES=true` in `.env` and run `docker compose up -d --wait` when people should find your sign-in page by searching for it. Compose passes the variable to the gateway and the console, which both read it; the Helm chart and the platform templates set it in one place too ([configuration](configuration.md#search-engines)). The **General** page of the instance settings shows which way it is set.

| What crawlers get | Off, the default | On |
| --- | --- | --- |
| `/robots.txt` | Every path disallowed | `/sign-in` and the paths of the files that draw it, such as `/assets/`, allowed; every other path disallowed |
| `X-Robots-Tag: noindex, nofollow` | On every response | On every response but that page and the files that exist under those paths |
| `<meta name="robots" content="noindex, nofollow">` | In every page | In every page but the sign-in page |

When it's on, these still stay out:

- `/sign-in` with a query, since its `next` names the page someone was on.
- `/sign-up`. Only the Platform API says whether your install takes sign-ups, and crawlers may not ask it, so they would see a form that an install by invitation doesn't offer.
- `/`, which sends a signed-out visitor to the sign-in page, and every page of the console behind it.
- Shared links, which are meant for the people they were sent to.
- The APIs, the MCP server and its sign-in. The gateway answers `noindex` on every response, and on its own address its `robots.txt` disallows every path.

`robots.txt` names no crawler, so its rules hold for every crawler that reads it, search engines and AI crawlers alike, such as Googlebot, OpenAI's GPTBot and OAI-SearchBot, Anthropic's ClaudeBot, PerplexityBot and Common Crawl's CCBot. Crawlers keep their copy for a day at most, and the console lets them keep it for an hour.

These are requests that well-behaved crawlers follow, not access control: everything but the sign-in pages needs a sign-in anyway. A search engine that obeys `robots.txt` never fetches a disallowed page, so it never reads that page's `noindex` either. So when other sites link to your install, it may still list the bare address, without a title or any text from it. An assistant that opens a link because someone asked it to may ignore `robots.txt` for that one visit, and sees what any signed-out visitor sees.

## Long streams

Proxies and load balancers close connections that carry no bytes for a while, often after 60 or 100 seconds. A model that reasons before it answers can be silent for longer. So while a stream is open and nothing was written for `KAESO_STREAMS_KEEPALIVE_INTERVAL` (15 seconds by default), the gateway writes a keepalive that the app's SDK ignores:

- An SSE comment, `: keepalive`, for Chat Completions and Responses.
- A `ping` event for Anthropic Messages.

Keep the interval well below the shortest idle timeout on the way. The gateway sends the reply's headers as soon as the provider answers, before the first event. Its own limits for a stream are `KAESO_UPSTREAM_FIRST_BYTE_TIMEOUT` and `KAESO_UPSTREAM_STREAM_IDLE_TIMEOUT` ([provider calls](configuration.md#provider-calls)).

[Request flow](../concepts/request-flow.md) explains what happens when a stream breaks off or the app goes away.

## Next steps

- [Email](email.md): Connect an SMTP server so sign-in links and invitations arrive.
- [Health and metrics](monitoring.md): Check the install's health and scrape its metrics.
