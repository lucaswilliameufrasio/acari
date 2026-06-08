# Changelog

All notable changes to this project will be documented in this file.

## [0.3.4] - 2026-06-08

### Bug Fixes

- Make acari project open TUI by default, add discovery tests

### Documentation

- Add upgrade section in README

### Features

- Project junk scanner with TUI management, I/O priority, and install-path fix

### Styling

- Cargo fmt

### Testing

- Add project CLI integration tests, docs, empty-state hints
## [0.3.3] - 2026-06-07

### Bug Fixes

- Change PR body instructions to git checkout main && git pull --rebase

### Chores

- Prepare for v0.3.2
- Prepare for v0.3.3

### Documentation

- Add git pull --release step in PR body
## [0.3.2] - 2026-06-07

### Bug Fixes

- Support v-prefixed version input, show version in workflow title
- Add workflows:write permission and explicit git add in prepare-release
- Remove invalid workflows permission and inputs.version from name
## [0.3.1] - 2026-06-07

### Bug Fixes

- Allow-dirty must be a list, not a string
- Regenerate Cargo.lock in Prepare Release workflow after version bump

### Chores

- Prepare for v0.3.1
## [0.3.0] - 2026-06-07

### Bug Fixes

- Add allow-dirty=ci to cargo-dist config 

### Chores

- Sync Cargo.lock after version bump to 0.2.0
- Prepare for v0.3.0
## [0.2.0] - 2026-06-05

### Bug Fixes

- Gate linux-only imports in distro.rs with cfg
- Add ACARI_CONFIG_HOME/ACARI_DATA_HOME env vars for cross-platform config
- Path traversal protection, exclude pattern safety, TOCTOU fix
- Set 0o600 permissions on config and history, log rotation, TOFU doc
- Format_bytes precision, timestamp saturation, exclude limits, clean handle tracking
- Is_safe_path uses exact match instead of starts_with, cargo fmt
- Restore ci.yml YAML structure 
- Replace broken cargo-install action with direct cargo install
- Add explicit base:main to create-pull-request action
- Replace create-pull-request action with gh CLI
- Remove --label release flag 
- Use unique branch names in prepare-release 
- Branch name uses -rc.N suffix, remove invalid --delete-branch flag

### CI / Build

- Pin GitHub Actions to SHAs, scope permissions, fix shell injection
- Bump Rust toolchain from 1.94 to 1.96
- Skip CI for documentation-only changes
- Add Prepare Release workflow for automated changelog + version bump
- Add environment approval and branch guard to Prepare Release

### Chores

- Add MIT license, fix release SHA, add third-party license compliance
- Prepare for v0.2.0

### Documentation

- Update releasing.md for cargo-dist workflow
- Add security glossary 
- Add SECURITY.md and CODEOWNERS for workflow protection
- Add CONTRIBUTING.md with commit and PR rules
- Clarify squash strategy in CONTRIBUTING.md
- Add PR template with checklist and sections
- Add branch naming CI check and start-task.sh script
- Make issue number optional in branch naming

### Features

- Human-readable bytes, persistent targets, i18n pt/en, better TUI
- Auto-generate CHANGELOG.md with git-cliff

### Other

- Rewrite start-task.sh as interactive summarizer

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
