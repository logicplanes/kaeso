FROM rust:1.99.1-slim-trixie@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7 AS chef
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
ARG PROFILE=release
COPY --from=plan /src/recipe.json recipe.json
RUN cargo chef cook --profile "${PROFILE}" --locked -p kaeso-testkit --bin kaeso-testkit --recipe-path recipe.json
COPY . .
RUN cargo build --profile "${PROFILE}" --locked -p kaeso-testkit --bin kaeso-testkit \
 && cp "target/${PROFILE}/kaeso-testkit" /kaeso-testkit \
 && rm -rf target

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:29ee70762214fe8190cea04ef1c0a84a8365879e047ce8f911ae5a5ab2355fa2 AS mock
COPY --from=mock-build --chmod=0555 /kaeso-testkit /usr/local/bin/kaeso-testkit
COPY --from=mock-build --chmod=0555 /src/crates/kaeso-codecs/fixtures /fixtures
USER nonroot:nonroot
ENV KAESO_KPP_FIXTURES=/fixtures
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/kaeso-testkit", "mock-provider", "0.0.0.0:8080"]

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:29ee70762214fe8190cea04ef1c0a84a8365879e047ce8f911ae5a5ab2355fa2 AS gateway
ARG VERSION=dev
LABEL org.opencontainers.image.title="Kaeso gateway" \
      org.opencontainers.image.description="The Kaeso gateway: one OpenAI-, Anthropic-, Gemini- and ElevenLabs-compatible API in front of every provider, with keys, alias routes, budgets, guardrails, request logs and an MCP portal under your control." \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.source="https://github.com/logicplanes/kaeso" \
      org.opencontainers.image.url="https://kaeso.app" \
      org.opencontainers.image.documentation="https://docs.kaeso.app" \
      org.opencontainers.image.licenses="MIT"
COPY --from=build --chmod=0555 /kaeso /usr/local/bin/kaeso
USER nonroot:nonroot
# Inside the container the metrics listen on every interface, so a port published to the
# host's loopback reaches them; Compose publishes 9100 nowhere and Caddy never routes it.
ENV KAESO_LISTEN=0.0.0.0:8080 KAESO_METRICS_LISTEN=0.0.0.0:9100
EXPOSE 8080 9100
HEALTHCHECK --interval=10s --timeout=5s --start-period=30s --start-interval=1s --retries=3 CMD ["/usr/local/bin/kaeso", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/kaeso"]
CMD ["serve"]
