FROM rust:1.99.0-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581 AS chef
RUN cargo install cargo-chef --version 0.1.78 --locked \
 && rm -rf /usr/local/cargo/registry
WORKDIR /src
ENV SQLX_OFFLINE=true

FROM chef AS plan
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
ARG PROFILE=release
COPY --from=plan /src/recipe.json recipe.json
RUN cargo chef cook --profile "${PROFILE}" --locked -p kaeso --recipe-path recipe.json
COPY . .
RUN cargo build --profile "${PROFILE}" --locked -p kaeso \
 && cp "target/${PROFILE}/kaeso" /kaeso \
 && rm -rf target

FROM chef AS mock-build
COPY --from=plan /src/recipe.json recipe.json
RUN cargo chef cook --release --locked -p kaeso-testkit --bin kaeso-testkit --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked -p kaeso-testkit --bin kaeso-testkit \
 && cp target/release/kaeso-testkit /kaeso-testkit \
 && rm -rf target

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:83b9b50586253821cf63ffcec87d3d94265a093ad30da232039af962dd8f4edf AS mock
COPY --from=mock-build --chmod=0555 /kaeso-testkit /usr/local/bin/kaeso-testkit
COPY --from=mock-build --chmod=0555 /src/crates/kaeso-codecs/fixtures /fixtures
USER nonroot:nonroot
ENV KAESO_KPP_FIXTURES=/fixtures
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/kaeso-testkit", "mock-provider", "0.0.0.0:8080"]

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:83b9b50586253821cf63ffcec87d3d94265a093ad30da232039af962dd8f4edf AS gateway
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
