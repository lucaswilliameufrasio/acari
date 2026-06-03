# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Bug Fixes

- Gate linux-only imports in distro.rs with cfg
- Add ACARI_CONFIG_HOME/ACARI_DATA_HOME env vars for cross-platform config
- Path traversal protection, exclude pattern safety, TOCTOU fix
- Set 0o600 permissions on config and history, log rotation, TOFU doc
- Format_bytes precision, timestamp saturation, exclude limits, clean handle tracking
- Is_safe_path uses exact match instead of starts_with, cargo fmt
- Restore ci.yml YAML structure 

### CI / Build

- Pin GitHub Actions to SHAs, scope permissions, fix shell injection
- Bump Rust toolchain from 1.94 to 1.96
- Skip CI for documentation-only changes

### Documentation

- Update releasing.md for cargo-dist workflow
- Add security glossary 
- Add SECURITY.md and CODEOWNERS for workflow protection

### Features

- Human-readable bytes, persistent targets, i18n pt/en, better TUI

### Styling

- Cargo fmt on all files
## [main] - 2026-03-24

### CI / Build

- Update GitHub Action versions

### Chores

- Add boilerplate

### Features

- Implement cross-platform scanner/cleaner with TUI and headless modes
- Add curl installer and release note install snippet
