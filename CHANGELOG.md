# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the crate uses
[Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-11-03

### Added

- `Client` (`Client::new`, `Client::builder`) with bearer auth, configurable
  base URL, timeout, retries, retry hooks and custom `reqwest::Client`.
- Retries on 408/409/425/429/502/503/504 (plus caller extras) and connection
  errors; `Retry-After` on 429; exponential backoff with full jitter otherwise.
- `truto::Error` with API, timeout, transport, decode and invalid-request
  variants and `is_not_found` / `is_rate_limited` / `is_unauthorized` helpers.
- Cursor `Pager` with `next_page`, `collect_all`, `first` and `into_stream`.
- `qs`-compatible query encoding and a `Query` builder.
- Typed models generated from the OpenAPI spec (`truto::models`), with open
  enums and on-demand union accessors.
- Admin resources (integrated accounts, integrations, environments, tenants,
  sync jobs, workflows, webhooks, datastores, logs, analytics, ...) and the
  Unified, Proxy, Custom and MCP APIs.
