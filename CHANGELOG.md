# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-03-24

### Added
- Interactive TUI scan/clean flow with `ratatui` + `tokio`.
- Headless mode with safe cleanup controls: `--clean`, `--dry-run`, and `--yes`.
- Custom ad-hoc scan targets via `--scan-path`.
- Cross-platform target catalog for macOS and Linux.
- Unit + integration + CLI tests covering scan, clean, dry-run, and CLI behavior.
- CI workflow for lint/build/test/docs.
- Release workflow for Linux/macOS/Windows binaries, including archive checksums (`.sha256`).
- Release documentation (`docs/releasing.md`) and checksum verification instructions in `README.md`.

### Changed
- Refactored into explicit Rust modules by responsibility (`application`, `domain`, `infrastructure`, `ui`).
- Headless flow and command orchestration centralized in application modules (`headless`, `commands`).
- Release packaging now includes both binaries: `acari` and `headless_cleaner`.

### Security
- Destructive headless cleanup now requires explicit confirmation (`--yes`).
