# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-01-30

### Added

- `--header` / `-H` CLI flag to pass custom HTTP headers to every Matomo API request (e.g. `--header "X-Auth:token" --header "X-Tenant:abc"`)
- CLI headers are merged with `MCP_MATOMO_EXTRA_HEADERS` environment variable, with CLI taking precedence

## [0.2.0] - 2026-01-23

### Added

- Custom `User-Agent` header (`mcp-matomo/<version>`) sent with every request
- `MCP_MATOMO_EXTRA_HEADERS` environment variable to inject custom HTTP headers into all Matomo API requests
- E2E test suite for Matomo API integration

### Changed

- Switched HTTP backend from native-tls to **rustls** for cross-compilation support (no more OpenSSL dependency)

## [0.1.0] - 2025-12-19

### Added

- Initial release
- MCP server exposing Matomo Analytics API as tools for Claude
- Dynamic API introspection: connects to a live Matomo instance and discovers all available API methods
- Static OpenAPI spec mode: load a pre-generated spec via `--openapi` flag
- Token-based authentication (`--token` / `MCP_MATOMO_TOKEN`)
- Site ID configuration (`--site-id` / `MCP_MATOMO_SITE_ID`)
- Stdio transport for Claude Desktop integration
- Cross-platform builds: Linux (x86_64), macOS (x86_64, aarch64), Windows (x86_64)

[0.3.0]: https://github.com/FGRibreau/mcp-matomo/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/FGRibreau/mcp-matomo/compare/mcp-matomo-v0.1.0...v0.2.0
[0.1.0]: https://github.com/FGRibreau/mcp-matomo/releases/tag/mcp-matomo-v0.1.0
