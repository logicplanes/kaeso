FROM rust:1.99.0-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581 AS build
WORKDIR /src
COPY . .
ENV SQLX_OFFLINE=true
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p kaeso \
 && cp target/release/kaeso /kaeso

FROM build AS mock-build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p kaeso-testkit --bin kaeso-testkit \
 && cp target/release/kaeso-testkit /kaeso-testkit

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2 AS mock
COPY --from=mock-build --chmod=0555 /kaeso-testkit /usr/local/bin/kaeso-testkit
COPY --from=mock-build --chmod=a=rX /src/crates/kaeso-codecs/fixtures /fixtures
USER nonroot:nonroot
ENV KAESO_KPP_FIXTURES=/fixtures
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/kaeso-testkit", "mock-provider", "0.0.0.0:8080"]

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2 AS gateway
ARG VERSION=dev
LABEL org.opencontainers.image.title="Kaeso gateway" \
      org.opencontainers.image.description="The Kaeso gateway: one OpenAI- and Anthropic-compatible API for your applications, with keys, routing, limits and usage under your control." \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.source="https://github.com/logicplanes/kaeso" \
      org.opencontainers.image.url="https://kaeso.app" \
      org.opencontainers.image.documentation="https://docs.kaeso.app" \
      org.opencontainers.image.licenses="MIT"
COPY --from=build --chmod=0555 /kaeso /usr/local/bin/kaeso
USER nonroot:nonroot
ENV KAESO_LISTEN=0.0.0.0:8080 KAESO_METRICS_LISTEN=0.0.0.0:9100
EXPOSE 8080 9100
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --retries=3 CMD ["/usr/local/bin/kaeso", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/kaeso"]
CMD ["serve"]
