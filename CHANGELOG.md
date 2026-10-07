# Changelog

All notable changes to this project will be documented in this file.

## [0.9.1] - 2026-10-07

### Bug Fixes

- Drop Intel macOS release target
- Refresh third-party license inventory
## [0.9.0] - 2026-10-07

### Bug Fixes

- Report privileged cleanup results accurately
- Validate privileged helper path ancestry
- Clarify cleanup previews and cache broker checks
- Require individual confirmation for custom cleanup targets
- Keep cleanup symlinks within selected scope
- Preserve cleanup roots and report cancellation accurately
- Report actual filesystem cleanup results
- Align desktop cleanup preview with execution scope
- Keep interrupted cleanup scans unconfirmed
- Preserve cleanup error details after cancellation
- Recover desktop UI from cleanup worker exit
- Report actual removed entry counts
- Honor cancellation before removing single files
- Validate exact targets before privileged cleanup
- Match desktop scan results by target path
- Reject overlapping desktop cleanup selections
- Resolve symlinked paths in cleanup overlap checks
- Normalize cleanup paths before overlap checks
- Block cleanup after incomplete desktop scans
- Require fresh scans before desktop cleanup
- Clarify symlink cleanup preview metrics
- Invalidate cleanup previews after execution
- Disable selection when cleanup preview expires
- Distinguish failed command estimates from zero
- Reject malformed cleanup estimate output
- Distinguish zero and unavailable cleanup estimates
- Bound Time Machine cleanup preview to operation
- Keep desktop cleanup UI polling active workers
- Bind cleanup confirmation to reviewed selection
- Update all duplicate cleanup scan rows
- Compare exact cleanup confirmation targets
- Label apt cleanup preview as approximate
- Qualify journal vacuum reclaim estimate
- Preserve partial Docker builder cleanup results
- Report unreadable Docker builder reclaim totals
- Stop simulator erase when shutdown fails
- Qualify simulator reset preview size
- Reject replaced cleanup targets
- Recheck cleanup identity during path resolution
- Preserve partial filesystem cleanup totals
- Include third-party license notices
- Normalize cleanup paths before overlap checks
- Prevent desktop breadcrumb navigation panic

### Chores

- Prepare for v0.9.0

### Documentation

- Describe desktop cleanup preview safety
- Qualify Time Machine cleanup estimate

### Features

- Add egui desktop disk analyzer
- Add treemap filtering and cleanup controls
- Use squarified disk treemap layout
- Add bulk target selection and filtering
- Improve treemap selection details
- Compare apparent and allocated disk size
- Add guarded desktop cleanup for allowlisted commands
- Add constrained Linux desktop privilege broker
- Add opt-in secure Linux helper installation
- Add constrained macOS desktop privilege flow
- Support safe Docker builder pruning in desktop UI
- Support safe iOS simulator reset in desktop UI
- Allow cancelling desktop cleanup operations
- Clarify desktop cleanup scope before confirmation
- Allow cancelling desktop cleanup scans
- Show cleanup scan progress per target
- Add disk analysis chart views
- Refine desktop analysis and cleanup interface

### Performance

- Build disk tree bottom-up
- Isolate desktop scans in rayon pool

### Testing

- Cover path-qualified cleanup scan progress
- Harden privileged helper path validation coverage
- Ensure special cleanup targets cannot run in batches
- Preserve cleanup preview after dry run
- Keep privileged cleanup results unmeasured
- Reject overlapping cleanup targets before execution
- Reject modified privileged cleanup targets
- Use portable paths for cleanup overlap
## [0.8.3] - 2026-09-28

### Bug Fixes

- Report actual volume prune results
- Stream prune output parsing
- Surface volume prune metric errors

### Chores

- Prepare for v0.8.3

### Documentation

- Add Acari release workflow skill
## [0.8.2] - 2026-09-28

### Bug Fixes

- Isolate errors and restore sudo terminal

### Chores

- Prepare for v0.8.2

### Testing

- Make cleanup fixtures portable across platforms
- Use safe paths for macOS scan fixtures
## [0.8.1] - 2026-09-15

### Chores

- Prepare for v0.8.1
- Prepare for v0.8.1
## [0.8.0] - 2026-09-15

### Bug Fixes

- Harden Docker cleanup and path validation
- Return failure when headless cleaning has errors

### Documentation

- Complete v0.8.0 changelog

### Features

- Cancel long-running cleanup commands
- Expand Docker cleanup targets
- Harden cleanup flows and coverage
## [0.7.2] - 2026-08-31

### Bug Fixes

- Drop duplicate rm -rf simulator wipe, flag iOS backups
- Exclude local volumes from prune estimate
- Use allocated result on non-Unix test path

### CI / Build

- Add cururu PR review workflow
- Feed analyzer evidence to cururu review

### Chores

- Bump toolchain to 1.98.0 and install cargo-about via install-action
- Prepare for v0.7.2

### Features

- Count nested and duplicate targets once in totals
- Mark dangerous targets in the target list
- Add --allocated-size for du-style block counting
- Used-space breakdown and du-style largest directories
- Cover Android, mise, FVM, Dart and Bun caches
- Add XDG user caches target on macOS
## [0.7.1] - 2026-08-24

### Bug Fixes

- Shutdown simulators before erase and skip os.update snapshots

### Chores

- Prepare for v0.7.1
## [0.7.0] - 2026-08-24

### Bug Fixes

- Dedupe command events, protect source dirs, add macOS caches
- Parse docker system df reclaimable size with 
- Gate macOS-only imports in scanner to fix non-macOS CI
- Add Windows GetDiskFreeSpaceExW support so df works on Windows

### CI / Build

- Use cargo-nextest for test execution
- Allow -rc.N suffix on release branch names

### Chores

- Add Makefile with setup, test, fmt, lint, check, run, release, clean targets
- Prepare for v0.7.0

### Documentation

- Add macOS System Data section to README
- Add ADR 003 dedicated rayon pool for directory traversal

### Features

- Delete_entire flag + tracked bytes in cleaner + 2 tests
- Add macOS targets for Xcode archives, iOS backups, simulator runtimes, Android Studio caches
- Add command targets for APFS Time Machine snapshots + new macOS targets
- Tests for command targets, TUI [cmd] badge, purgeable space parser
- Add Docker prune, apt autoremove, journalctl vacuum command targets
- Add requires_sudo and dangerous metadata to CleanTarget
- Dangerous confirmation phase + [sudo] badge in TUI
- Json output, history/df commands, TUI search, new caches

### Performance

- Cache diskutil query, parallel simctl scan, avoid alloc in parse_human_size
- Dedicated rayon pool per scan, avoid jwalk global-pool abort

### Refactor

- Extract exec module with parsers + 25 tests
- Explicit TargetOrigin for targets, fix custom --list label

### Styling

- Fix fmt and clippy warnings in scanner and tests
## [0.6.0] - 2026-06-11

### Bug Fixes

- Auto-sort targets by size, cursor in Finished phase, sorted indicator

### Chores

- Prepare for v0.6.0

### Testing

- Add 3 tests for auto-sort behavior on scan finish
## [0.5.1] - 2026-06-10

### Chores

- Prepare for v0.5.1

### Features

- Scroll in scan TUI + shared resolve_scroll + 4 visible_list tests
## [0.5.0] - 2026-06-10

### Chores

- Prepare for v0.5.0

### Features

- Auto-scroll in project TUI + 11 scroll/confirmation tests
## [0.4.0] - 2026-06-10

### Bug Fixes

- Use numeric index instead of name in pattern removal confirmation

### Chores

- Prepare for v0.4.0
## [0.3.11] - 2026-06-10

### Bug Fixes

- Cleaner symlinks, partial bytes, macOS uchg; TUI pattern cursor/confirm/validate

### Chores

- Prepare for v0.3.11

### Features

- Add clear-patterns subcommand

### Testing

- Add_pattern validation, clear-patterns CLI, broken symlink cleaner
## [0.3.10] - 2026-06-10

### Bug Fixes

- Show live bytes from in-progress targets in gauge label
- Use unique relative names for project junk dirs, add gauge label tests
- Use MAIN_SEPARATOR for Windows compatibility in project scan names

### Chores

- Prepare for v0.3.10
## [0.3.9] - 2026-06-10

### Bug Fixes

- Strip top-level directory when extracting archive

### Chores

- Prepare for v0.3.9
## [0.3.8] - 2026-06-10

### Bug Fixes

- Remove version tag from asset name in install script

### Chores

- Prepare for v0.3.8
## [0.3.7] - 2026-06-10

### Bug Fixes

- Change archive extension from tar.gz to tar.xz
- Correct taiki-e/install-action SHA

### CI / Build

- Use cargo-binstall for fast git-cliff installation
- Replace cargo-binstall with taiki-e/install-action for git-cliff

### Chores

- Prepare for v0.3.7

### Documentation

- Fix README install commands with correct repo path
## [0.3.6] - 2026-06-10

### Bug Fixes

- Show live byte count in gauge and discovery progress

### Chores

- Prepare for v0.3.6
## [0.3.5] - 2026-06-09

### Chores

- Prepare for v0.3.5

### Features

- Install.sh detects and removes old binaries in PATH
## [0.3.4] - 2026-06-08

### Bug Fixes

- Make acari project open TUI by default, add discovery tests
- Expand tilde in project scan roots, add discovery feedback

### Chores

- Prepare for v0.3.4

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
## [0.0.1] - 2026-04-27

### CI / Build

- Update GitHub Action versions

### Chores

- Add boilerplate

### Features

- Implement cross-platform scanner/cleaner with TUI and headless modes
- Add curl installer and release note install snippet
