FROM node:24.21.0-alpine AS build
WORKDIR /app
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm fetch --frozen-lockfile
COPY . .
RUN pnpm install --offline --frozen-lockfile --filter kaeso-console...
RUN pnpm --filter kaeso-console... build \
  && rm -rf apps/console/dist/.vite \
  && find apps/console/dist -name '*.map' -delete

FROM caddy:2.11.7-alpine
ARG VERSION=0.1.0
LABEL org.opencontainers.image.title="Kaeso console" \
      org.opencontainers.image.description="The web console for the Kaeso gateway." \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.source="https://github.com/logicplanes/kaeso" \
      org.opencontainers.image.url="https://kaeso.app" \
      org.opencontainers.image.documentation="https://docs.kaeso.app" \
      org.opencontainers.image.licenses="MIT"
RUN addgroup -S console && adduser -S -G console -H -s /sbin/nologin console \
  && mkdir -p /var/lib/console/data /var/lib/console/config \
  && chown -R console:console /var/lib/console
ENV XDG_DATA_HOME=/var/lib/console/data XDG_CONFIG_HOME=/var/lib/console/config
COPY deploy/console/Caddyfile /etc/caddy/Caddyfile
COPY --from=build /app/apps/console/dist /srv
USER console
EXPOSE 3000
HEALTHCHECK --interval=15s --timeout=3s --start-period=5s CMD ["wget", "-qO-", "http://127.0.0.1:3000/healthz"]
CMD ["caddy", "run", "--config", "/etc/caddy/Caddyfile", "--adapter", "caddyfile"]
