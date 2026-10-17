FROM rust:1.99.0-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581 AS build
WORKDIR /src
COPY . .
ENV SQLX_OFFLINE=true
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p kaeso-updater \
 && cp target/release/kaeso-updater /kaeso-updater \
 && mkdir -p /dirs/kaeso /dirs/maintenance /dirs/state

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2
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
