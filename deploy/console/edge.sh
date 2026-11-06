#!/bin/sh
set -eu
for name in KAESO_GATEWAY_UPSTREAM KAESO_CONSOLE_UPSTREAM; do
  eval "value=\${$name:-}"
  if [ -z "$value" ]; then
    echo "kaeso-edge: set $name to the host and port the edge forwards to, such as kaeso-gateway:8080" >&2
    exit 1
  fi
done
exec caddy run --config /etc/caddy/edge.Caddyfile --adapter caddyfile
