# caddy:2.11.7-alpine is built with Go 1.26.8 and x/net 0.59.0; Go 1.26.9 and x/net 0.60.0 fix
# GO-2026-6603, GO-2026-6609, GO-2026-6610, GO-2026-6611, GO-2026-6612 and GO-2026-6617.
FROM golang:1.27.2-alpine@sha256:f92b6ef800e499660581efdabdf25d9d817a9d124eaf900924f0504e7e27e12d AS caddy
ENV CGO_ENABLED=0 GOTOOLCHAIN=local XCADDY_GO_BUILD_FLAGS="-trimpath -ldflags=-buildid="
RUN go install github.com/caddyserver/xcaddy/cmd/xcaddy@v0.4.7 \
  && xcaddy build v2.11.7 --output /caddy --replace golang.org/x/net=golang.org/x/net@v0.60.0 \
  && go version -m /caddy | grep -qE '^\s+(dep|=>)\s+golang\.org/x/net\s+v0\.60\.0\s'

FROM node:24.22.1-alpine@sha256:06d7adf3e0ef61a13f1d28682726366f97a15aa1888c934d3ecec86c33918b53 AS build
WORKDIR /app
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm fetch --frozen-lockfile
COPY . .
RUN pnpm install --offline --frozen-lockfile --filter kaeso-console...
RUN pnpm --filter kaeso-console... build \
  && rm -rf apps/console/dist/.vite \
  && find apps/console/dist -name '*.map' -delete

FROM alpine:3.24.3@sha256:be2656061b536c1883c4034790e21d0a09c5aaaf51968b89368a3c0371ea2c9a
ARG VERSION=dev
LABEL org.opencontainers.image.title="Kaeso console" \
      org.opencontainers.image.description="The web console for the Kaeso gateway." \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.source="https://github.com/logicplanes/kaeso" \
      org.opencontainers.image.url="https://kaeso.app" \
      org.opencontainers.image.documentation="https://docs.kaeso.app" \
      org.opencontainers.image.licenses="MIT"
RUN apk add --no-cache ca-certificates mailcap \
  && addgroup -S console && adduser -S -G console -H -s /sbin/nologin console \
  && mkdir -p /etc/caddy /srv /var/lib/console/data /var/lib/console/config \
  && chown -R console:console /var/lib/console
ENV XDG_DATA_HOME=/var/lib/console/data XDG_CONFIG_HOME=/var/lib/console/config
COPY --from=caddy --chmod=0555 /caddy /usr/bin/caddy
COPY --chmod=0444 deploy/console/Caddyfile /etc/caddy/Caddyfile
COPY --chmod=0555 deploy/console/serve.sh /usr/local/bin/kaeso-console
COPY --from=build --chmod=0555 /app/apps/console/dist /srv
USER console
RUN caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
EXPOSE 3000
HEALTHCHECK --interval=15s --timeout=3s --start-period=5s CMD ["wget", "-qO-", "http://127.0.0.1:3000/healthz"]
CMD ["/usr/local/bin/kaeso-console"]
