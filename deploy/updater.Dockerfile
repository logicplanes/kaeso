FROM rust:1.99.0-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581 AS chef
RUN cargo install cargo-chef --version 0.1.78 --locked \
 && rm -rf /usr/local/cargo/registry
WORKDIR /src
ENV SQLX_OFFLINE=true

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

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:83b9b50586253821cf63ffcec87d3d94265a093ad30da232039af962dd8f4edf
ARG VERSION=dev
LABEL org.opencontainers.image.title="Kaeso updater" \
      org.opencontainers.image.description="The optional Kaeso updater: verifies a Kaeso release and switches a Docker Compose install to it." \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.source="https://github.com/logicplanes/kaeso" \
      org.opencontainers.image.url="https://kaeso.app" \
      org.opencontainers.image.documentation="https://docs.kaeso.app" \
      org.opencontainers.image.licenses="MIT"
COPY --from=build --chmod=0555 /kaeso-updater /usr/local/bin/kaeso-updater
COPY --from=build --chown=65532:65532 --chmod=0755 /dirs/ /
USER nonroot:nonroot
EXPOSE 8081
HEALTHCHECK --interval=10s --timeout=5s --start-period=10s --retries=3 CMD ["/usr/local/bin/kaeso-updater", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/kaeso-updater"]
