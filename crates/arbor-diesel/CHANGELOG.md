# Changelog

All notable changes to `arbor-diesel` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-14

### Added

- `Hierarchy`, `Scope`, `ScopeKey`, `Direction`, and `Traversal` for typed `WITH RECURSIVE` descendant and ancestor traversals on Diesel (`sqlite`, `postgres`, and `libsql` features).
- `creates_cycle` and `reached_depth_limit` helpers for database-backed cycle checks and depth truncation detection.
