#!/bin/sh
set -eu
if ! unreadable="$(find /etc/caddy/Caddyfile /srv ! -exec test -r {} \; -print 2>&1)" || [ -n "$unreadable" ]; then
  echo "kaeso-console: user $(id -un) cannot read these files, so the console cannot start: $unreadable" >&2
  exit 1
fi
# The gateway's own words for a bool, so one variable means the same to both.
case "$(printf '%s' "${KAESO_SEARCH_ENGINES:-}" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')" in
  true | 1 | yes | on) KAESO_SEARCH_ENGINES=true ;;
  '' | false | 0 | no | off) KAESO_SEARCH_ENGINES=false ;;
  *)
    echo "kaeso-console: KAESO_SEARCH_ENGINES must be true or false, as for the gateway, not \"$KAESO_SEARCH_ENGINES\"" >&2
    exit 1
    ;;
esac
export KAESO_SEARCH_ENGINES
exec caddy run --config /etc/caddy/Caddyfile --adapter caddyfile
