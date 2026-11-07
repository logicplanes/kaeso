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
  install_docker="${KAESO_INSTALL_DOCKER:-}"
  open_browser=yes
  [ "${KAESO_NO_OPEN:-}" != yes ] || open_browser=no
  while [ $# -gt 0 ]; do
    case "$1" in
      --domain) domain="${2:?--domain needs a value}"; shift 2 ;;
      --dir) dir="${2:?--dir needs a value}"; shift 2 ;;
      --version) version="${2:?--version needs a value}"; shift 2 ;;
      --install-docker) install_docker=yes; shift ;;
      --no-open) open_browser=no; shift ;;
      -h | --help) usage; return 0 ;;
      *) fail "Unknown option $1. Run with --help to see the options." ;;
    esac
  done

  command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || fail "The installer needs curl or wget."
  docker_ready
  if [ -n "$domain" ]; then
    cleaned="$(clean_domain "$domain")" || fail "\"$domain\" is not a domain. Give one such as kaeso.example.com, or localhost."
    domain="$cleaned"
  fi

  umask 077
  project="${COMPOSE_PROJECT_NAME:-kaeso}"
  created=
  [ -d "$dir" ] || created=yes
  mkdir -p "$dir"
  cd "$dir"
  here="$(pwd -P)"
  only_install "$project" "$here" || {
    status=$?
    cd - >/dev/null
    [ -z "$created" ] || rmdir "$here" 2>/dev/null || true
    exit "$status"
  }

  fresh=
  if [ -f .env ]; then
    current="$(sed -n 's/^KAESO_DOMAIN=//p' .env | head -n 1)"
    if [ -n "$domain" ] && [ "$domain" != "$current" ]; then
      note "This install keeps its domain $current. To change it, set KAESO_DOMAIN in .env and run docker compose up -d."
    fi
    domain="$current"
    say "Kaeso is installed in $here; starting it"
  else
    fresh=yes
    [ -n "$domain" ] || domain="$(choose_domain)"
    check_dns "$domain"
    say "Installing Kaeso in $here"
  fi

  if [ ! -f compose.yaml ]; then
    if [ "$version" = latest ]; then
      url="https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml"
    else
      url="https://github.com/logicplanes/kaeso/releases/download/v${version#v}/compose.yaml"
    fi
    url="${KAESO_COMPOSE_URL:-$url}"
    download "$url" compose.yaml.part || {
      rm -f compose.yaml.part
      fail "Couldn't download $url. Check the version and the connection, and run the installer again."
    }
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

  pin_subnet "$project"

  if [ ! -f compose.override.yaml ] && [ -z "$(docker compose ps -q --status running caddy </dev/null 2>/dev/null)" ]; then
    free_ports
  fi

  images="$(docker compose config --images </dev/null)"
  gateway="$(printf '%s\n' "$images" | grep -m1 'kaeso-gateway')"
  console="$(printf '%s\n' "$images" | grep -m1 'kaeso-console')"

  missing=
  for image in $images; do
    docker image inspect "$image" >/dev/null 2>&1 </dev/null || missing=yes
  done
  if [ -n "$missing" ]; then
    say "Downloading Kaeso's images, about 2 GB; this takes a minute or two"
    quiet=--quiet
    [ ! -t 2 ] || quiet=
    docker compose pull $quiet </dev/null || fail "Couldn't download Kaeso's images. Check the connection and run the installer again."
  fi

  # Docker Desktop, Colima and OrbStack share only some folders, such as your home folder, with their Linux machine.
  docker run --rm --user 0:0 -v "$here:/kaeso:ro" --entrypoint test "$console" -f /kaeso/compose.yaml </dev/null 2>/dev/null ||
    fail "Docker can't see $here, so it can't start Kaeso from there. Install Kaeso in your home folder, or share this folder with Docker in its settings."

  if [ ! -s master-keys ]; then
    say "Creating the master key"
    docker run --rm "$gateway" master-key generate </dev/null >master-keys.part
    mv master-keys.part master-keys
    # The gateway reads the key as user 65532. Docker Desktop, Colima and OrbStack read the folder as you instead.
    if [ "$(uname -s)" = Darwin ] || docker info --format '{{.OperatingSystem}}' </dev/null | grep -q 'Docker Desktop'; then
      chmod 0400 master-keys
    else
      docker run --rm --user 0:0 -v "$here:/kaeso" --entrypoint sh "$console" \
        -c "chown 65532:$(id -g) /kaeso/master-keys && chmod 0440 /kaeso/master-keys" </dev/null
    fi
  fi

  say "Starting Kaeso"
  log="$(mktemp)"
  if ! docker compose up -d --wait --wait-timeout 900 </dev/null >"$log" 2>&1; then
    explain_failure "$log"
  fi
  rm -f "$log"

  certificate_ready || explain_certificate

  setup="$(docker compose exec -T gateway kaeso setup-link </dev/null 2>&1 || true)"
  link="$(printf '%s\n' "$setup" | grep -m1 -oE 'https?://[^[:space:]]*/setup#token=[^[:space:]]+' || true)"
  if [ -n "$link" ]; then
    say "Kaeso is running. Open this link to create the first user:"
    echo
    printf '%s\n' "$setup"
    open_link "$link"
  else
    say "Kaeso is running at $(address). Sign in there."
  fi
  echo

  if [ "$domain" = localhost ]; then
    echo "Your browser asks you once to accept the certificate Caddy signed for localhost."
    over_ssh_hint
  fi
  if [ -n "$fresh" ]; then
    echo "Emails are only written to the gateway's log. To send them, add your SMTP server to .env:"
    echo "https://docs.kaeso.app/self-hosting/email"
    echo
    echo "Keep a copy of $here/master-keys apart from your backups: without it, stored provider keys can't be decrypted."
    echo
    echo "Next, add a provider key and create an API key in the console:"
    echo "https://docs.kaeso.app/get-started/quick-start#add-a-provider-key"
  fi
}

usage() {
  cat <<'EOF'
Installs Kaeso with Docker Compose into a new folder and starts it.

Usage: curl -fsSL https://kaeso.app/install.sh | sh -s -- [options]

  --domain DOMAIN    the domain Kaeso answers on (default: localhost)
  --dir DIR          the folder to install into (default: ./kaeso)
  --version VERSION  the release to install (default: the newest stable release)
  --install-docker   on Linux, install Docker Engine first if it's missing, without asking
  --no-open          don't open the setup link in the browser

The environment variables KAESO_DOMAIN, KAESO_INSTALL_DIR, KAESO_VERSION, KAESO_INSTALL_DOCKER=yes and KAESO_NO_OPEN=yes
set the same options.
Running it again in the same folder keeps the settings and the master key, and starts Kaeso if it's stopped.
When Docker Desktop, OrbStack or Colima is installed but stopped, the installer starts it.
EOF
}

DEFAULT_SUBNET=172.30.83.0/24

say() { printf '\n==> %s\n' "$1"; }

note() { printf 'kaeso install: %s\n' "$1" >&2; }

fail() {
  note "$1"
  exit 1
}

can_ask() { (: </dev/tty) 2>/dev/null; }

ask() {
  printf '%s' "$1" >/dev/tty
  read -r answer </dev/tty || answer=
}

download() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 -o "$2" "$1"
  else
    wget -q -O "$2" "$1"
  fi
}

secret() { od -An -N24 -tx1 /dev/urandom | tr -d ' \n'; }

at_least() {
  major="${1%%.*}"
  rest="${1#*.}"
  minor="${rest%%.*}"
  [ "$major" -gt "$2" ] 2>/dev/null || { [ "$major" -eq "$2" ] 2>/dev/null && [ "$minor" -ge "$3" ] 2>/dev/null; }
}

docker_ready() {
  if ! command -v docker >/dev/null 2>&1; then
    [ "$(uname -s)" = Linux ] || fail "Docker isn't installed. Install Docker Desktop, OrbStack or Colima, start it and run the installer again: https://docs.docker.com/get-started/get-docker/"
    if [ "$install_docker" != yes ]; then
      can_ask || fail "Docker isn't installed. Install Docker Engine (https://docs.docker.com/engine/install/), or run the installer with --install-docker to install it first."
      ask "Docker isn't installed. Install Docker Engine now with Docker's script from get.docker.com? [Y/n] "
      case "$answer" in
        "" | y | Y | yes | Yes) ;;
        *) fail "Install Docker Engine (https://docs.docker.com/engine/install/), then run the installer again." ;;
      esac
    fi
    get_docker
  fi

  if ! info="$(docker info 2>&1 </dev/null)"; then
    case "$info" in
      *"permission denied"*)
        fail "$(id -un) can't use Docker. Add it to the docker group with sudo usermod -aG docker $(id -un), log in again and run the installer again." ;;
      *)
        if ! start_docker; then
          if [ "$(uname -s)" = Linux ] && command -v systemctl >/dev/null 2>&1; then
            fail "Docker isn't running. Start it with sudo systemctl start docker and run the installer again."
          fi
          fail "Docker isn't running. Start it and run the installer again."
        fi ;;
    esac
  fi

  compose_version="$(docker compose version --short 2>/dev/null </dev/null | sed 's/^v//')" || compose_version=
  [ -n "$compose_version" ] ||
    fail "Docker Compose isn't installed. Kaeso needs Compose 2.24 or newer: https://docs.docker.com/compose/install/"
  at_least "$compose_version" 2 24 || fail "Kaeso needs Docker Compose 2.24 or newer; this is $compose_version."

  arch="$(docker info --format '{{.Architecture}}' </dev/null)"
  case "$arch" in
    x86_64 | amd64 | aarch64 | arm64) ;;
    *) fail "Kaeso's images run on x86_64 and arm64, and this Docker runs on $arch." ;;
  esac

  # The documented minimum is 4 GB; Docker reports a little less than the machine has.
  memory="$(docker info --format '{{.MemTotal}}' </dev/null)"
  if [ "$memory" -gt 0 ] 2>/dev/null && [ "$memory" -lt 3500000000 ]; then
    more="give Docker more"
    case "$(docker context show 2>/dev/null </dev/null)" in
      colima*) more="give Colima more with colima stop and colima start --memory 4" ;;
    esac
    note "Docker has $((memory / 1000000)) MB of memory, and Kaeso needs 4 GB. ClickHouse may run out of memory: $more, or turn request logs off (https://docs.kaeso.app/self-hosting/installation#turn-request-logs-off)."
  fi
}

# Docker Desktop, OrbStack and Colima don't always start with the computer. The Docker context names the one in use.
start_docker() {
  case "$(uname -s)" in
    Darwin)
      context="$(docker context show 2>/dev/null </dev/null)" || context=
      profile=
      case "$context" in
        orbstack) engine=orbstack ;;
        colima) engine=colima ;;
        colima-*) engine=colima profile="${context#colima-}" ;;
        desktop-linux) engine=desktop ;;
        *)
          if command -v orb >/dev/null 2>&1; then
            engine=orbstack
          elif command -v colima >/dev/null 2>&1; then
            engine=colima
          elif [ -d /Applications/Docker.app ] || [ -d "$HOME/Applications/Docker.app" ]; then
            engine=desktop
          else
            return 1
          fi ;;
      esac
      case "$engine" in
        orbstack)
          command -v orb >/dev/null 2>&1 || return 1
          say "Starting OrbStack"
          orb start </dev/null >/dev/null 2>&1 || return 1 ;;
        colima)
          command -v colima >/dev/null 2>&1 || return 1
          say "Starting Colima; this takes a minute"
          colima start ${profile:+--profile "$profile"} </dev/null >/dev/null 2>&1 || return 1 ;;
        desktop)
          say "Starting Docker Desktop; this takes a minute"
          open -a Docker </dev/null >/dev/null 2>&1 || return 1 ;;
      esac ;;
    Linux)
      [ "$(id -u)" -eq 0 ] && command -v systemctl >/dev/null 2>&1 || return 1
      say "Starting Docker"
      systemctl start docker </dev/null >/dev/null 2>&1 || return 1 ;;
    *) return 1 ;;
  esac
  tries=0
  until info="$(docker info 2>&1 </dev/null)"; do
    tries=$((tries + 1))
    [ "$tries" -lt 90 ] || return 1
    sleep 2
  done
}

get_docker() {
  sudo=
  if [ "$(id -u)" -ne 0 ]; then
    command -v sudo >/dev/null 2>&1 || fail "Installing Docker needs root. Run the installer as root, or install Docker Engine first: https://docs.docker.com/engine/install/"
    sudo=sudo
  fi
  say "Installing Docker Engine with Docker's script from get.docker.com; this takes a minute"
  script="$(mktemp)"
  log="$(mktemp)"
  download https://get.docker.com "$script" || fail "Couldn't download Docker's script from https://get.docker.com."
  if ! $sudo sh "$script" </dev/null >"$log" 2>&1; then
    tail -n 20 "$log" >&2
    rm -f "$script" "$log"
    fail "Docker's script couldn't install Docker on this system. Install Docker Engine by hand: https://docs.docker.com/engine/install/"
  fi
  rm -f "$script" "$log"
  if [ -n "$sudo" ]; then
    $sudo usermod -aG docker "$(id -un)"
    fail "Docker is installed, and $(id -un) is now in the docker group. Log out and in again, then run the installer again."
  fi
}

# One Kaeso per Docker host: its Compose project has a fixed name, so a second install would take over the first one.
only_install() {
  elsewhere="$(docker ps -a --filter "label=com.docker.compose.project=$1" --format '{{.Label "com.docker.compose.project.working_dir"}}' </dev/null | head -n 1)"
  if [ -n "$elsewhere" ] && [ "$(cd "$elsewhere" 2>/dev/null && pwd -P)" != "$2" ]; then
    if [ -d "$elsewhere" ]; then
      note "Kaeso is already installed on this machine, in $elsewhere. Run the installer there, or remove that install first: https://docs.kaeso.app/self-hosting/installation#remove-kaeso"
    else
      note "Kaeso is already installed on this machine, from $elsewhere, which $(id -un) can't open. Run the installer there as that folder's owner. Only if that folder is gone, remove the old install with docker compose -p $1 down -v, which deletes its data."
    fi
    return 1
  fi
  if [ ! -f .env ] && docker volume ls -q --filter "label=com.docker.compose.project=$1" </dev/null | grep -q .; then
    note "The data of an earlier Kaeso install is still in Docker, without the .env whose passwords open it. Run the installer in that install's folder, or delete the data with docker compose -p $1 down -v and run it again."
    return 1
  fi
}

# The edge network needs a range no other Docker network overlaps, such as one of Docker's own /16 pools.
free_subnet() {
  used="$(docker network ls -q </dev/null | xargs docker network inspect --format '{{range .IPAM.Config}}{{.Subnet}} {{end}}' 2>/dev/null)" || used=
  for candidate in "$DEFAULT_SUBNET" 172.31.83.0/24 10.83.30.0/24 10.183.30.0/24 192.168.183.0/24; do
    if echo "$used" | tr ' ' '\n' | awk -v c="$candidate" '
      function ip(s, a) { split(s, a, "."); return ((a[1] * 256 + a[2]) * 256 + a[3]) * 256 + a[4] }
      function lo(n, p) { split(n, p, "/"); return ip(p[1]) - ip(p[1]) % 2 ^ (32 - p[2]) }
      function hi(n, p) { split(n, p, "/"); return lo(n) + 2 ^ (32 - p[2]) - 1 }
      /^[0-9.]+\/[0-9]+$/ { if (lo($0) <= hi(c) && lo(c) <= hi($0)) clash = 1 }
      END { exit clash }'; then
      echo "$candidate"
      return 0
    fi
  done
  return 1
}

# Until the edge network exists, nothing depends on its range, so an install that never started can still move it.
pin_subnet() {
  grep -q '^EDGE_SUBNET=' .env && return 0
  docker network inspect "$1_edge" >/dev/null 2>&1 </dev/null && return 0
  subnet="$(free_subnet)" || return 0
  [ "$subnet" = "$DEFAULT_SUBNET" ] || echo "EDGE_SUBNET=$subnet" >>.env
}

clean_domain() {
  d="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' -e 's#^[a-z][a-z0-9+.-]*://##' -e 's#/.*$##')"
  case "$d" in
    # A bare port is Caddy's site address for plain HTTP behind another proxy.
    :[0-9]*) case "${d#:}" in *[!0-9]*) return 1 ;; esac ;;
    "" | *[!a-z0-9.-]* | .* | *. | -* | *..*) return 1 ;;
  esac
  printf '%s\n' "$d"
}

choose_domain() {
  can_ask || {
    echo localhost
    return 0
  }
  while :; do
    ask "Domain Kaeso answers on [localhost]: "
    [ -n "$answer" ] || answer=localhost
    clean_domain "$answer" && return 0
    printf '"%s" is not a domain. Give one such as kaeso.example.com, or localhost.\n' "$answer" >/dev/tty
  done
}

# Caddy signs these itself instead of asking a public certificate authority.
own_certificate() {
  case "$1" in
    localhost | *.localhost | *.local | *.internal | *.home.arpa | :*) return 0 ;;
    *[!0-9.]*) return 1 ;;
    *) return 0 ;;
  esac
}

resolves() {
  if command -v getent >/dev/null 2>&1; then
    getent hosts "$1" >/dev/null 2>&1
  elif command -v dscacheutil >/dev/null 2>&1; then
    dscacheutil -q host -a name "$1" 2>/dev/null | grep -q address
  elif command -v host >/dev/null 2>&1; then
    host "$1" >/dev/null 2>&1
  else
    return 0
  fi
}

check_dns() {
  own_certificate "$1" && return 0
  resolves "$1" && return 0
  if can_ask; then
    ask "$1 has no DNS record yet, so Caddy can't get its certificate. Install anyway? [y/N] "
    case "$answer" in
      y | Y | yes | Yes) return 0 ;;
      *) fail "Point an A or AAAA record for $1 at this server, wait until it resolves, and run the installer again. To try Kaeso first, use --domain localhost." ;;
    esac
  fi
  note "$1 has no DNS record yet, so Caddy can't get its certificate. Point an A or AAAA record at this server; Caddy keeps trying until it resolves."
}

# Asks Caddy on this machine, so a router that can't loop back to its own public address doesn't matter.
certificate_ready() {
  own_certificate "$domain" && return 0
  [ -z "$(sed -n 's/^KAESO_PUBLIC_URL=//p' .env | head -n 1)" ] || return 0
  command -v curl >/dev/null 2>&1 || return 0
  tries=0
  until curl -fsS -o /dev/null --max-time 5 --resolve "$domain:443:127.0.0.1" "https://$domain/healthz" </dev/null 2>/dev/null; do
    tries=$((tries + 1))
    [ "$tries" -lt 30 ] || return 1
    sleep 2
  done
}

explain_certificate() {
  note "Caddy hasn't got a certificate for $domain yet, so browsers can't open Kaeso there."
  answer="$(docker compose logs --no-log-prefix --tail 200 caddy </dev/null 2>/dev/null | grep 'could not get certificate' | sed -n 's/.*"error":"\([^"]*\)".*/\1/p' | tail -n 1)"
  [ -z "$answer" ] || note "The certificate authority answered: $answer"
  note "Check that the DNS record of $domain points at this server and that ports 80 and 443 are open to the internet. Caddy keeps trying; docker compose logs caddy in $here shows its progress."
}

open_link() {
  [ "$open_browser" = yes ] && [ -t 1 ] && [ -z "${SSH_CONNECTION:-}" ] || return 0
  if [ "$(uname -s)" = Darwin ]; then
    opener=open
  elif { [ -n "${DISPLAY:-}" ] || [ -n "${WAYLAND_DISPLAY:-}" ]; } && command -v xdg-open >/dev/null 2>&1; then
    opener=xdg-open
  else
    return 0
  fi
  if "$opener" "$1" </dev/null >/dev/null 2>&1; then
    echo
    echo "It's open in your browser."
  fi
}

address() {
  url="$(sed -n 's/^KAESO_PUBLIC_URL=//p' .env | head -n 1)"
  if [ -n "$url" ]; then
    echo "$url"
  else
    case "$domain" in
      :*) echo "http://localhost$domain" ;;
      *) echo "https://$domain" ;;
    esac
  fi
}

port_taken() {
  command -v curl >/dev/null 2>&1 || return 1
  curl -s -o /dev/null --connect-timeout 2 "http://127.0.0.1:$1/" </dev/null
  case $? in
    7 | 28) return 1 ;;
    *) return 0 ;;
  esac
}

# Ports 80 and 443 are often taken on a computer, so a localhost install moves to the first free pair; a real domain
# needs them for its certificate.
free_ports() {
  case "$domain" in :*) return 0 ;; esac
  port_taken 80 || port_taken 443 || return 0
  if [ "$domain" != localhost ]; then
    port=443
    ! port_taken 80 || port=80
    fail "Port $port on this machine is taken by another program, so Caddy can't serve Kaeso or get its certificate. Stop that program, or let it forward to Kaeso (https://docs.kaeso.app/self-hosting/domains-and-tls#use-your-own-reverse-proxy), and run the installer again."
  fi
  for https in 8443 9443 10443 11443; do
    http=$((https - 363))
    if ! port_taken "$https" && ! port_taken "$http"; then
      cat >compose.override.yaml <<EOF
# Ports 80 and 443 were taken when Kaeso was installed, so Caddy answers on these.
services:
  caddy:
    ports: !override
      - "$http:80"
      - "$https:443"
      - "$https:443/udp"
EOF
      grep -q '^KAESO_PUBLIC_URL=' .env || echo "KAESO_PUBLIC_URL=https://localhost:$https" >>.env
      note "Ports 80 and 443 are taken on this machine, so Kaeso answers on https://localhost:$https."
      return 0
    fi
  done
  fail "Ports 80 and 443 are taken on this machine, and so are 8443, 9443, 10443 and 11443. Free one of them and run the installer again."
}

over_ssh_hint() {
  [ -n "${SSH_CONNECTION:-}" ] || return 0
  echo "You're connected over SSH, and https://localhost only opens in a browser on this machine."
  echo "To use Kaeso from your computer, point a domain at this server, set KAESO_DOMAIN in .env to it and run docker compose up -d in $here."
  server="$(printf '%s\n' "$SSH_CONNECTION" | awk '{print $3}')"
  case "$server" in
    10.* | 127.* | 192.168.* | 172.1[6-9].* | 172.2[0-9].* | 172.3[01].* | *:*) ;;
    *.*.*.*) echo "Without a domain of your own, $(printf '%s' "$server" | tr . -).sslip.io points at this server." ;;
  esac
  echo
}

explain_failure() {
  if grep -qE 'address already in use|port is already allocated' "$1"; then
    port="$(grep -oE '[0-9.]*:(80|443)[^0-9]' "$1" | grep -oE '(80|443)' | head -n 1)"
    rm -f "$1"
    docker compose down </dev/null >/dev/null 2>&1 || true
    fail "Port ${port:-80 or 443} on this machine is taken by another program, so Caddy can't serve Kaeso. Stop that program, or let it forward to Kaeso (https://docs.kaeso.app/self-hosting/domains-and-tls#use-your-own-reverse-proxy), and run the installer again."
  fi
  if grep -q 'Pool overlaps' "$1"; then
    rm -f "$1"
    fail "Another Docker network uses the range of Kaeso's edge network. Set EDGE_SUBNET in .env to a free range, such as 10.83.31.0/24, and run the installer again."
  fi
  tail -n 5 "$1" >&2
  rm -f "$1"
  for service in $(docker compose ps -a --format '{{.Service}}|{{.State}}|{{.Health}}|{{.ExitCode}}' </dev/null | awk -F'|' '($2 != "running" && $4 != "0") || $3 == "unhealthy" || $2 == "restarting" {print $1}' | sort -u); do
    printf '\n%s, the last lines of its log:\n' "$service" >&2
    docker compose logs --no-log-prefix --tail 15 "$service" </dev/null >&2 || true
  done
  fail "Kaeso didn't start. Fix the cause above and run the installer again; docker compose logs in $here shows more."
}

main "$@"
