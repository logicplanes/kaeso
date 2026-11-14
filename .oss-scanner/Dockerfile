# The build environment for Anthropic's OSS Scanner (https://github.com/anthropics/oss-scanner). The scanner builds
# this file with the repository as its context and then audits the image with no network, so every toolchain,
# dependency and database the tests need is installed here. threat_model.md says how to use the image.
FROM rust:1.99.0-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581

ARG DEBIAN_FRONTEND=noninteractive
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
      build-essential ca-certificates clang cmake curl git gnupg jq lld pkg-config procps python3 python3-venv xz-utils \
    && curl -fsSL https://www.postgresql.org/media/keys/ACCC4CF8.asc | gpg --dearmor -o /usr/share/keyrings/pgdg.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/pgdg.gpg] https://apt.postgresql.org/pub/repos/apt trixie-pgdg main" \
      > /etc/apt/sources.list.d/pgdg.list \
    && curl -fsSL https://packages.clickhouse.com/rpm/lts/repodata/repomd.xml.key | gpg --dearmor -o /usr/share/keyrings/clickhouse.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/clickhouse.gpg] https://packages.clickhouse.com/deb stable main" \
      > /etc/apt/sources.list.d/clickhouse.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends postgresql-18 clickhouse-server clickhouse-client valkey-server \
    && rm -rf /var/lib/apt/lists/* \
    && printf '%s' '<clickhouse><users><kaeso><password>kaeso</password><networks><ip>127.0.0.1</ip><ip>::1</ip></networks>' \
      '<profile>default</profile><quota>default</quota><access_management>1</access_management></kaeso></users></clickhouse>' \
      > /etc/clickhouse-server/users.d/kaeso.xml

ENV COREPACK_HOME=/usr/local/corepack \
    COREPACK_ENABLE_DOWNLOAD_PROMPT=0 \
    PATH=/usr/local/node/bin:$PATH \
    SQLX_OFFLINE=true \
    CARGO_INCREMENTAL=0 \
    CARGO_TERM_COLOR=never \
    KAESO_TEST_POSTGRES_URL=postgres://kaeso:kaeso@127.0.0.1:5432/kaeso \
    KAESO_TEST_CLICKHOUSE_URL=http://kaeso:kaeso@127.0.0.1:8123 \
    KAESO_TEST_VALKEY_URL=redis://127.0.0.1:6379

# Full debug info makes the workspace's test binaries several times larger; line tables keep file and line in backtraces.
ENV CARGO_PROFILE_DEV_DEBUG=line-tables-only \
    CARGO_PROFILE_TEST_DEBUG=line-tables-only

COPY .node-version package.json /tmp/node/
RUN rustup component add rustfmt clippy llvm-tools \
    && rustup toolchain install nightly --profile minimal \
    && cargo install --locked cargo-nextest --version 0.9.151 \
    && cargo install --locked cargo-fuzz --version 0.13.2 \
    && rm -rf "$CARGO_HOME/registry" \
    && cd /tmp/node \
    && version="$(cat .node-version)" \
    && file="node-v${version}-linux-$(dpkg --print-architecture | sed 's/amd64/x64/').tar.xz" \
    && curl -fsSLO "https://nodejs.org/dist/v${version}/${file}" \
    && curl -fsSL "https://nodejs.org/dist/v${version}/SHASUMS256.txt" | grep " ${file}\$" | sha256sum -c - \
    && mkdir -p /usr/local/node \
    && tar -xJf "$file" -C /usr/local/node --strip-components=1 \
    && corepack enable \
    && corepack install \
    && rm -rf /tmp/node

COPY . /src
WORKDIR /src

RUN pnpm install --frozen-lockfile \
    && python3 -m venv e2e/.venv \
    && e2e/.venv/bin/pip install --no-cache-dir --require-hashes -r e2e/requirements.txt

RUN cargo build --workspace --all-features --locked \
    && cargo nextest run --workspace --all-features --locked --no-run

# The fuzz workspace builds with nightly and AddressSanitizer; a broken nightly must not stop the scan.
RUN cargo fetch --manifest-path fuzz/Cargo.toml --locked \
    && (cargo +nightly fuzz build --target "$(rustc -vV | sed -n 's/^host: //p')" \
      || echo "WARNING: the fuzz targets did not build")

RUN chmod +x .oss-scanner/services \
    && .oss-scanner/services start \
    && (cargo nextest run --locked --no-fail-fast -p kaeso-core -p kaeso-crypto -p kaeso-kpp -p kaeso-codecs -p kaeso-store \
      || echo "WARNING: some tests failed") \
    && .oss-scanner/services stop
