<div align="center">
  <br>
  <a href="https://kaeso.app#gh-light-mode-only"><img src="https://raw.githubusercontent.com/logicplanes/kaeso/dev/.github/assets/kaeso-logo-light.svg#gh-light-mode-only" alt="Kaeso" width="320"></a>
  <a href="https://kaeso.app#gh-dark-mode-only"><img src="https://raw.githubusercontent.com/logicplanes/kaeso/dev/.github/assets/kaeso-logo-dark.svg#gh-dark-mode-only" alt="Kaeso" width="320"></a>
  <br>
  <br>
  <p>The open-source AI gateway you run yourself.<br>One OpenAI- and Anthropic-compatible API in front of your providers, with keys, limits and logs under your control.</p>
  <p>
    <a href="https://github.com/logicplanes/kaeso/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/logicplanes/kaeso/actions/workflows/ci.yml/badge.svg?branch=dev"></a>
    <a href="LICENSE"><img alt="License: MIT" src=".github/assets/badges/license.svg"></a>
  </p>
  <p>
    <a href="https://kaeso.app"><b>Website</b></a> ·
    <a href="https://docs.kaeso.app"><b>Documentation</b></a> ·
    <a href="https://docs.kaeso.app/get-started/quick-start"><b>Quick start</b></a> ·
    <a href="https://docs.kaeso.app/development/contributing"><b>Contributing</b></a> ·
    <a href="CHANGELOG.md"><b>Changelog</b></a>
  </p>
  <br>
</div>

<img alt="A walk through the Kaeso console: connecting an Anthropic provider key, creating an API key, sending a request to claude-haiku-5-5 from a terminal, and the request arriving at the top of the request logs" src="https://raw.githubusercontent.com/logicplanes/kaeso/dev/.github/assets/demo-light.gif#gh-light-mode-only">
<img alt="A walk through the Kaeso console: connecting an Anthropic provider key, creating an API key, sending a request to claude-haiku-5-5 from a terminal, and the request arriving at the top of the request logs" src="https://raw.githubusercontent.com/logicplanes/kaeso/dev/.github/assets/demo-dark.gif#gh-dark-mode-only">

## Quick start

```bash
mkdir -m 700 kaeso && cd kaeso
curl -fsSLO https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml
cat > .env <<EOF
KAESO_DOMAIN=localhost
KAESO_MAIL_TRANSPORT=log
POSTGRES_PASSWORD=$(openssl rand -hex 24)
POSTGRES_SUPERUSER_PASSWORD=$(openssl rand -hex 24)
CLICKHOUSE_PASSWORD=$(openssl rand -hex 24)
VALKEY_PASSWORD=$(openssl rand -hex 24)
EOF
docker compose run --rm --no-deps gateway master-key generate > master-keys
chmod 644 master-keys
docker compose up -d --wait
docker compose exec gateway kaeso setup-link
```

Open the link it prints to create the first user, then follow the [quick start](https://docs.kaeso.app/get-started/quick-start) to run Kaeso on your own domain, connect a provider and send the first request.

## License

[MIT](LICENSE). The Kaeso name and logo are not covered by the license. Third-party notices are in [NOTICE.md](NOTICE.md).
