#!/bin/sh
set -eu
if ! unreadable="$(find /etc/caddy/Caddyfile /srv ! -exec test -r {} \; -print 2>&1)" || [ -n "$unreadable" ]; then
  echo "kaeso-console: user $(id -un) cannot read these files, so the console cannot start: $unreadable" >&2
  exit 1
fi
exec caddy run --config /etc/caddy/Caddyfile --adapter caddyfile
