#!/bin/sh
# Installs Kaeso with Docker Compose into a new folder and starts it:
#
#   curl -fsSL https://kaeso.app/install.sh | sh
#
# Run it with --help for the options. Running it again in the same place keeps the settings and the master key.
set -eu

# Wrapped in a function so that a download cut off halfway runs nothing.
main() {
  domain="${KAESO_DOMAIN:-}"
  dir="${KAESO_INSTALL_DIR:-kaeso}"
  version="${KAESO_VERSION:-latest}"
  while [ $# -gt 0 ]; do
    case "$1" in
      --domain) domain="${2:?--domain needs a value}"; shift 2 ;;
      --dir) dir="${2:?--dir needs a value}"; shift 2 ;;
      --version) version="${2:?--version needs a value}"; shift 2 ;;
      -h | --help) usage; return 0 ;;
      *) fail "Unknown option $1. Run with --help to see the options." ;;
    esac
  done

  need curl
  need docker
  docker info >/dev/null 2>&1 </dev/null || fail "Docker isn't running, or this user can't reach it. Start Docker and run the installer again."
  compose_version="$(docker compose version --short 2>/dev/null </dev/null | sed 's/^v//')" ||
    fail "Docker Compose isn't installed. Kaeso needs Compose 2.24 or newer."
  at_least "$compose_version" 2 24 || fail "Kaeso needs Docker Compose 2.24 or newer; this is $compose_version."

  if [ -z "$domain" ]; then
    domain=localhost
    if (: </dev/tty) 2>/dev/null; then
      printf 'Domain Kaeso answers on [localhost]: ' >/dev/tty
      read -r answer </dev/tty || answer=
      [ -n "$answer" ] && domain="$answer"
    fi
  fi

  umask 077
  mkdir -p "$dir"
  cd "$dir"
  say "Installing Kaeso in $(pwd)"

  if [ ! -f compose.yaml ]; then
    if [ "$version" = latest ]; then
      url="https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml"
    else
      url="https://github.com/logicplanes/kaeso/releases/download/v${version#v}/compose.yaml"
    fi
    curl -fsSL --retry 3 -o compose.yaml.part "${KAESO_COMPOSE_URL:-$url}" || fail "Couldn't download $url."
    mv compose.yaml.part compose.yaml
  fi

  if [ ! -f .env ]; then
    {
      echo "KAESO_DOMAIN=$domain"
      echo "POSTGRES_PASSWORD=$(secret)"
      echo "POSTGRES_SUPERUSER_PASSWORD=$(secret)"
      echo "CLICKHOUSE_PASSWORD=$(secret)"
      echo "VALKEY_PASSWORD=$(secret)"
      echo "KAESO_MAIL_TRANSPORT=log"
    } >.env.part
    mv .env.part .env
  fi

  images="$(docker compose config --images </dev/null)"
  gateway="$(printf '%s\n' "$images" | grep -m1 'kaeso-gateway')"
  console="$(printf '%s\n' "$images" | grep -m1 'kaeso-console')"

  if [ ! -s master-keys ]; then
    say "Creating the master key"
    docker run --rm "$gateway" master-key generate </dev/null >master-keys.part
    mv master-keys.part master-keys
    # The gateway reads the key as user 65532. Docker Desktop, Colima and OrbStack read the folder as you instead.
    if [ "$(uname -s)" = Darwin ] || docker info --format '{{.OperatingSystem}}' </dev/null | grep -q 'Docker Desktop'; then
      chmod 0400 master-keys
    else
      docker run --rm --user 0:0 -v "$(pwd):/kaeso" --entrypoint sh "$console" \
        -c 'chown 65532:65532 /kaeso/master-keys && chmod 0400 /kaeso/master-keys' </dev/null
    fi
  fi

  say "Starting Kaeso; the first start downloads the images and takes a minute or two"
  docker compose up -d --wait </dev/null

  say "Kaeso is running. Open this link to create the first user:"
  echo
  docker compose exec -T gateway kaeso setup-link </dev/null || echo "Print a setup link with: docker compose exec gateway kaeso setup-link"
  echo
  if [ "$domain" = localhost ]; then
    echo "Your browser asks you once to accept the certificate Caddy signed for localhost."
  fi
  echo "Emails are only written to the gateway's log. To send them, add your SMTP server to .env:"
  echo "https://docs.kaeso.app/self-hosting/email"
  echo
  echo "Keep a copy of $(pwd)/master-keys apart from your backups: without it, stored provider keys can't be decrypted."
}

usage() {
  cat <<'EOF'
Installs Kaeso with Docker Compose into a new folder and starts it.

Usage: curl -fsSL https://kaeso.app/install.sh | sh -s -- [options]

  --domain DOMAIN    the domain Kaeso answers on (default: localhost)
  --dir DIR          the folder to install into (default: ./kaeso)
  --version VERSION  the release to install (default: the newest stable release)

The environment variables KAESO_DOMAIN, KAESO_INSTALL_DIR and KAESO_VERSION set the same options.
EOF
}

say() { printf '\n==> %s\n' "$1"; }

fail() {
  printf 'kaeso install: %s\n' "$1" >&2
  exit 1
}

need() { command -v "$1" >/dev/null 2>&1 || fail "$1 isn't installed. Kaeso needs Docker with Compose, and curl."; }

secret() { od -An -N24 -tx1 /dev/urandom | tr -d ' \n'; }

at_least() {
  major="${1%%.*}"
  rest="${1#*.}"
  minor="${rest%%.*}"
  [ "$major" -gt "$2" ] 2>/dev/null || { [ "$major" -eq "$2" ] 2>/dev/null && [ "$minor" -ge "$3" ] 2>/dev/null; }
}

main "$@"
