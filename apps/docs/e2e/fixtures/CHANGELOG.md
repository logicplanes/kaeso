# Changelog

All notable changes to Kaeso are documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Batches for the model API, with their results kept for 30 days.

### Fixed

- Quiet streams end after the stream-idle timeout instead of waiting for the total timeout.

## [1.3.0-rc.1] - 2026-12-01

### Added

- A batch page in the console.

## [1.2.0] - 2026-11-15

Aliases across projects, and request logs that keep less by default.

### Added

#### Model API

- Aliases that point at a model in another project, set in the [catalogue](https://docs.kaeso.app/console/models-and-aliases):
  - one alias per project,
  - resolved before the limits are checked.
- `kaeso-model` on every streamed response, so an app sees which model answered
  even when it asked for an alias.

#### Console

- A filter for aliases on the models page.

### Changed

- Request logs keep 30 days by default; [the retention][retention] can still be set per organization.

### Deprecated

- `ttl` in batch requests; send `expires_after` instead.

### Removed

- The `KAESO_LOG_LEGACY` setting.

### Security

- Sign-in links expire after 15 minutes.

## [1.1.1] - 2026-10-08 [YANKED]

### Fixed

- Budgets count streamed requests that end early.

## [1.1.0] - 2026-10-01

### Fixed

- Spend budgets round to whole cents, as [1.0.0](#v1.0.0) documented.

## [1.0.0] - 2026-09-01

The first stable release.

### Added

- The gateway, the console and the documentation.

[Unreleased]: https://github.com/logicplanes/kaeso/compare/v1.3.0-rc.1...HEAD
[1.3.0-rc.1]: https://github.com/logicplanes/kaeso/releases/tag/v1.3.0-rc.1
[1.2.0]: https://github.com/logicplanes/kaeso/releases/tag/v1.2.0
[1.1.1]: https://github.com/logicplanes/kaeso/releases/tag/v1.1.1
[1.1.0]: https://github.com/logicplanes/kaeso/releases/tag/v1.1.0
[1.0.0]: https://github.com/logicplanes/kaeso/releases/tag/v1.0.0
[retention]: https://docs.kaeso.app/console/request-logs
