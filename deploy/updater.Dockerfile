# Docker Compose v5.6.1, Apache-2.0, run by the updater's compose runner; checksums from the release's checksums.txt.
FROM scratch AS compose-download
ADD --checksum=sha256:47737f4236e6d3ccea7291995203f8f018058783b4c3a9469119811a0b5db8c0 \
    https://github.com/docker/compose/releases/download/v5.6.1/docker-compose-linux-x86_64 /amd64/docker-compose
ADD --checksum=sha256:42cfa75f51ad0ccf290bad37899e0038a8c10a0069e85233ecc1d53e3ff1347d \
    https://github.com/docker/compose/releases/download/v5.6.1/docker-compose-linux-aarch64 /arm64/docker-compose
ADD --checksum=sha256:58d1e17ffe5109a7ae296caafcadfdbe6a7d176f0bc4ab01e12a689b0499d8bd \
    https://raw.githubusercontent.com/docker/compose/v5.6.1/LICENSE /licenses/docker-compose/LICENSE
ADD --checksum=sha256:b7dca0a6b01fa7365e4892877a6321179ee343d72ee87a96cfc222141b99a1e6 \
    https://raw.githubusercontent.com/docker/compose/v5.6.1/NOTICE /licenses/docker-compose/NOTICE

FROM rust:1.99.0-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581 AS chef
RUN cargo install cargo-chef --version 0.1.78 --locked \
 && rm -rf /usr/local/cargo/registry
WORKDIR /src
ENV SQLX_OFFLINE=true

# Picks the binary for the target architecture here rather than with a stage named by a variable,
# so every FROM names a pinned image or a stage.
FROM chef AS compose
ARG TARGETARCH
COPY --from=compose-download / /download/
RUN install -D -m 0555 "/download/${TARGETARCH}/docker-compose" /out/docker-compose \
 && cp -R /download/licenses /out/licenses

FROM chef AS plan
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=plan /src/recipe.json recipe.json
RUN cargo chef cook --release --locked -p kaeso-updater --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked -p kaeso-updater \
 && cp target/release/kaeso-updater /kaeso-updater \
 && rm -rf target \
 && mkdir -p /dirs/kaeso /dirs/maintenance /dirs/state

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2
ARG VERSION=dev
LABEL org.opencontainers.image.title="Kaeso updater" \
      org.opencontainers.image.description="The optional Kaeso updater: verifies a Kaeso release and switches a Docker Compose install to it." \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.source="https://github.com/logicplanes/kaeso" \
      org.opencontainers.image.url="https://kaeso.app" \
      org.opencontainers.image.documentation="https://docs.kaeso.app" \
      org.opencontainers.image.licenses="MIT AND Apache-2.0"
COPY --from=build --chmod=0555 /kaeso-updater /usr/local/bin/kaeso-updater
COPY --from=compose /out/docker-compose /usr/local/bin/docker-compose
COPY --from=compose /out/licenses/ /usr/share/doc/
COPY --from=build --chown=65532:65532 --chmod=0755 /dirs/ /
USER nonroot:nonroot
EXPOSE 8081
HEALTHCHECK --interval=10s --timeout=5s --start-period=10s --start-interval=1s --retries=3 CMD ["/usr/local/bin/kaeso-updater", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/kaeso-updater"]
