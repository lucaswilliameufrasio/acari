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

## 🚀 Getting Started

### Prerequisites
* [Rust toolchain](https://rustup.rs/) (1.94+)

### Installation

Quick install from GitHub Releases (curl | sh):

```bash
curl -fsSL https://raw.githubusercontent.com/<owner>/<repo>/main/scripts/install.sh | \
  sh -s -- --repo <owner>/<repo>
```

Install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/<owner>/<repo>/main/scripts/install.sh | \
  sh -s -- --repo <owner>/<repo> --tag v0.1.0
```

Build from source:

```bash
git clone [https://github.com/your-username/acari.git](https://github.com/your-username/acari.git)
cd acari
cargo run --release
```

### Usage

Launch the TUI (interactive mode):

```bash
acari
```

Navigate the interface using your keyboard:

* `<Space>`: Toggle selection of a junk category.
* `<Enter>`: Confirm and aggressively clean selected targets.
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
