# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the crate uses
[Semantic Versioning](https://semver.org/).

## Unreleased

### Fixed

- Listing the full integration catalog no longer fails with "invalid type: sequence, expected a map" or "invalid type: null, expected a string".
  - A resource method's `body` is now `Option<Value>`, because some integrations (greythr) send a JSON array there.
  - Maps whose values the spec marks nullable (integration `headers`, override `resources`) now hold `Option<..>`, so a `null` header (which removes an inherited header) decodes and round-trips (nativebridge).
- The documented log types were wrong (`api_call` is rejected). They are now `unified_proxy_api`, `rapid_bridge`, `webhook`, `mcp` and `sync_job_cron_trigger`.
- The README's integrated-account filter example used a nested object that the API rejects. It now uses the dotted key `integration.name`.

### Changed

- Fixes to the published spec now live in `spec/overlay.json` (OpenAPI Overlay), applied when regenerating models.

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
