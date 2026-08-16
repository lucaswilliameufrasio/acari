# 🐟 Acarí (Acari Cleaner)

> A blazing-fast, fearless disk cleaner for macOS and Linux, built in Rust.

Standard disk analyzers often choke on macOS System Integrity Protection (SIP) errors or fail to calculate the true size of Docker virtual disks and APFS snapshots. Acarí is built differently. It dives deep into your system's hidden directories, utilizing parallel traversal to instantly find and purge gigabytes of dead cache, orphaned containers, and build artifacts.

## 🤔 Why "Acarí"?

In the Amazon basin, the **Acarí** (also known as the armored catfish or *cascudo*) is a resilient bottom-dwelling fish. It naturally clings to the deepest, most inaccessible parts of its environment, relentlessly vacuuming up dirt, algae, and waste that nothing else will touch.

This TUI does exactly the same thing to your SSD. It ignores the superficial files and dives straight into the dark, forgotten depths of `~/Library/Caches`, `.cargo/registry`, and orphaned Docker volumes to suck up the junk holding your storage hostage.

## ✨ Features

* **Parallel Traversal:** Powered by `jwalk`, Acarí scans massive, deeply nested directories across multiple threads.
* **Non-Blocking TUI:** Built with `ratatui` and `tokio`, keeping the interface responsive during scans.
* **Headless Mode for Automation:** Supports `--headless` for terminal-only workflows and CI scripts.
* **Safe Cleaning Controls:** `--clean` now requires `--yes` for destructive runs, with `--dry-run` to simulate cleanup without deleting anything.
* **Custom Scan Paths:** Add ad-hoc directories with `--scan-path` for focused scans and tests.
* **Permission-Aware:** Handles permission failures safely and reports cleanup errors per target.
* **Environment Variables:**
  - `ACARI_CONFIG_HOME`: override config directory (`$ACARI_CONFIG_HOME/acari/config.toml`)
  - `ACARI_DATA_HOME`: override data directory (`$ACARI_DATA_HOME/acari/history.log`)
  - `XDG_CONFIG_HOME`: fallback config directory (Linux, if `ACARI_CONFIG_HOME` not set)

## 🍎 macOS "System Data"

macOS's **System Data** (formerly "Other" in older versions) is a catch-all category for files that don't fit into Apps, Photos, or Backups. The largest contributors are:

| Source | Typical size | Acarí target |
|---|---|---|
| **APFS Time Machine local snapshots** | 5–100 GB | `Time Machine Local Snapshots` |
| **Xcode DerivedData** | 1–20 GB | `Xcode DerivedData` |
| **Xcode Archives** | 1–10 GB | `Xcode Archives` |
| **iOS Simulator Runtimes** | 5–30 GB each | `iOS Simulator Runtimes` |
| **iOS Device Backups** | 5–50 GB | `iOS Device Backups` |
| **iOS DeviceSupport** | 1–5 GB | `iOS DeviceSupport` |
| **User Caches + Logs** | 1–10 GB | `User Caches`, `User Logs` |
| **Trash** | 0.1–50 GB | `Trash` |
| **XDG/Homebrew caches** | 0.5–5 GB | `Homebrew Cache`, etc. |
| **Android Studio SDK + Caches** | 10–50 GB | `Google IDE Caches` (all versions) |

Run `acari` — all detected targets appear with their sizes. Select what you want and press Enter to clean. Command targets (like APFS snapshots) show a `[cmd]` badge and execute system commands instead of deleting files.

**Note:** APFS snapshots may require `sudo`. Enter your password when prompted.

### Developer, Apple & App Caches

Beyond the classic caches, Acarí tracks modern toolchain and application caches
that can silently consume gigabytes:

| Category | Targets |
|---|---|
| JS/Python runtimes | `Bun Cache`, `Deno Cache`, `pnpm Cache`, `Yarn Cache`, `pip Cache`, `NPM Cache` |
| Rust / Go / .NET | `Cargo Registry`, `Cargo Git Checkouts`, `Go Build Cache`, `Go Module Cache`, `NuGet Cache` |
| Apple toolchain | `CocoaPods Cache`, `SwiftPM Cache`, `macOS Diagnostic Reports`, `iOS Simulator Devices`, `iOS Simulators Reset` |
| Apps | `Spotify Cache`, `Slack Cache`, `Discord Cache`, `VS Code Cache`, `VS Code ShipIt Cache` |

`iOS Simulators Reset` is a dangerous command target that runs
`xcrun simctl erase all` to wipe every local simulator device.

## 🚀 Getting Started

### Prerequisites
* [Rust toolchain](https://rustup.rs/) (1.96+)

### Installation

Quick install from GitHub Releases (curl | sh):

```bash
curl -fsSL https://raw.githubusercontent.com/lucaswilliameufrasio/acari/main/scripts/install.sh | \
  sh -s -- --repo lucaswilliameufrasio/acari
```

Install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/lucaswilliameufrasio/acari/main/scripts/install.sh | \
  sh -s -- --repo lucaswilliameufrasio/acari --tag v0.1.0
```

Build from source:

```bash
git clone https://github.com/lucaswilliameufrasio/acari.git
cd acari
cargo run --release
```

### Upgrade

Re-run the installer to overwrite the existing binary:

```bash
curl -fsSL https://raw.githubusercontent.com/lucaswilliameufrasio/acari/main/scripts/install.sh | \
  sh -s -- --repo lucaswilliameufrasio/acari
```

Or build from source:

```bash
cargo install --git https://github.com/lucaswilliameufrasio/acari
```

Verify:

```bash
acari --version
```

### Usage

Launch the TUI (interactive mode):

```bash
acari
```

Navigate the interface using your keyboard:

* `<Space>`: Toggle selection of a junk category.
* `<Enter>`: Confirm and aggressively clean selected targets.
* `a`: Select / deselect all.
* `i`: Invert the current selection.
* `c`: Clear (deselect) all.
* `s`: Cycle the sort order (size desc/asc, name, file count).
* `/`: Start an interactive search/filter by name.
* `d`: Toggle dry-run mode.
* `q` or `<Esc>`: Exit the application gracefully.

Run headless scan:

```bash
acari --headless
```

Headless scan + safe dry-run cleanup:

```bash
acari --headless --clean --dry-run
```

Headless destructive cleanup (explicit confirmation required):

```bash
acari --headless --clean --yes
```

Scan only a custom path:

```bash
acari --headless --target target-that-does-not-exist --scan-path /tmp/my-cache
```

### CLI Reference

```bash
# Disk usage overview (fast, no full scan)
acari df

# Show the cleanup history log
acari history

# Clear the cleanup history log
acari history --clear

# Structured JSON output for scripting (headless)
acari --headless --json
```

`acari df` shows the primary volume's total/used/free space plus the APFS
purgeable space (macOS). Use `acari --headless` to get the full reclaimable
estimate across all detected caches. `--json` is also available on
`acari project scan --headless --json` for integration with scripts, status
bars (Waybar, SwiftBar, Raycast) and CI.

### Project Junk Scanner

Find and clean build/cache directories across your projects (node_modules, target, build, .venv, __pycache__, etc.):

```bash
# Open the project management TUI (add/remove roots and patterns, launch scan)
acari project

# Direct scan one or more project roots
acari project scan ~/projects ~/work

# Scan with patterns and dry-run
acari project scan --no-default-patterns --pattern .terraform --headless

# Scan with excluded directories (also pruned during discovery)
acari project scan --exclude node_modules --exclude vendor ~/projects

# Emit JSON for scripting
acari project scan --headless --json ~/projects

# Manage project roots
acari project add-root ~/projects
acari project list-roots
acari project remove-root ~/projects

# Add custom junk directory patterns
acari project add-pattern .terraform
acari project list-patterns
acari project remove-pattern .terraform
```

The built-in patterns (30 total) include: `node_modules`, `target`, `build`, `.next`, `__pycache__`, `.venv`, `vendor`, `.gradle`, `.svelte-kit`, `.parcel-cache`, `.docusaurus`, `.angular`, `.serverless`, and more. Custom patterns can be added via CLI or the TUI.

### Verify Release Checksums

Each release asset includes:
- `acari` (TUI + headless mode),
- `headless_cleaner` (headless-only binary),
- a matching `.sha256` checksum file for the archive.

Linux/macOS:

```bash
sha256sum -c acari-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz.sha256
```

If `sha256sum` is unavailable on macOS:

```bash
shasum -a 256 -c acari-vX.Y.Z-aarch64-apple-darwin.tar.gz.sha256
```

Windows (PowerShell):

```powershell
$asset = "acari-vX.Y.Z-x86_64-pc-windows-msvc.zip"
$expected = (Get-Content "$asset.sha256").Split(" ")[0].Trim().ToLower()
$actual = (Get-FileHash -Algorithm SHA256 -Path $asset).Hash.ToLower()
if ($expected -eq $actual) { "OK" } else { "MISMATCH" }
```

### Release Process

- Changelog: [CHANGELOG.md](./CHANGELOG.md)
- Release guide: [docs/releasing.md](./docs/releasing.md)

## 🏗️ Architecture

Acarí uses a strictly layered architecture tailored for Text User Interfaces, ensuring the UI never blocks and the OS-specific quirks remain isolated.

* **UI (`src/ui`)**: Pure Ratatui components and event loop.
* **Application (`src/application`)**: Orchestration modules (`scanner`, `cleaner`, `headless`, `commands`) and `mpsc` state management.
* **Domain (`src/domain`)**: Core data structures (`CleanTarget`, `ScanResult`) and target composition (`targets`, `custom_targets`).
* **Infrastructure (`src/infrastructure`)**: OS-specific file system operations, `jwalk` integration, and Docker socket queries.
