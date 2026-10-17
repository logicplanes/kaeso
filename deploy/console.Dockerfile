FROM node:24.21.0-alpine@sha256:ebfe2f90462722a7a4de65e91990e97fe0d401c70e0e762c5b53302f905ec1c1 AS build
WORKDIR /app
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm fetch --frozen-lockfile
COPY . .
RUN pnpm install --offline --frozen-lockfile --filter kaeso-console...
RUN pnpm --filter kaeso-console... build \
  && rm -rf apps/console/dist/.vite \
  && find apps/console/dist -name '*.map' -delete

FROM caddy:2.11.7-alpine@sha256:d8542f48d34a9cf4e4c11a478865229840e87e4c96ea3f439101f31a5d35f75f
ARG VERSION=dev
LABEL org.opencontainers.image.title="Kaeso console" \
      org.opencontainers.image.description="The web console for the Kaeso gateway." \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.source="https://github.com/logicplanes/kaeso" \
      org.opencontainers.image.url="https://kaeso.app" \
      org.opencontainers.image.documentation="https://docs.kaeso.app" \
      org.opencontainers.image.licenses="MIT"
RUN apk add --no-cache --virtual .setcap libcap-utils \
  && setcap -r /usr/bin/caddy \
  && apk del .setcap \
  && addgroup -S console && adduser -S -G console -H -s /sbin/nologin console \
  && mkdir -p /var/lib/console/data /var/lib/console/config \
  && chown -R console:console /var/lib/console
ENV XDG_DATA_HOME=/var/lib/console/data XDG_CONFIG_HOME=/var/lib/console/config
COPY --chmod=0444 deploy/console/Caddyfile /etc/caddy/Caddyfile
COPY --chmod=0555 deploy/console/serve.sh /usr/local/bin/kaeso-console
COPY --from=build --chmod=a=rX /app/apps/console/dist /srv
USER console
RUN caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
EXPOSE 3000
HEALTHCHECK --interval=15s --timeout=3s --start-period=5s CMD ["wget", "-qO-", "http://127.0.0.1:3000/healthz"]
CMD ["/usr/local/bin/kaeso-console"]
