# Security & Architecture Glossary

Terms used across the codebase and security audit.

---

## TOCTOU (Time Of Check, Time Of Use)

A race condition where a resource is checked (e.g., `symlink_metadata`) and then used (e.g., `remove_file`), but an attacker modifies the resource in between.

**Example:** A file at `~/.cache/foo/temp.dat` is scanned as a regular file. Between the scan and the clean phase, an attacker replaces it with a symlink to `/etc/passwd`. If the cleaner follows the symlink, it deletes the target instead of the symlink.

**Mitigation in acari:** `cleaner.rs` calls `safe_canonicalize()` on each entry before deletion, verifying the resolved path still lies within the target's root. If the path escapes, the entry is skipped with an error.

---

## TOFU (Trust On First Use)

A trust model where the first interaction with a service establishes trust that is assumed for all subsequent interactions.

**Example:** `scripts/install.sh` downloads both the binary archive AND its SHA256 checksum from the same GitHub Releases source. This protects against accidental corruption but NOT against a compromised release — an attacker who controls the release can replace both files.

**Mitigation:** For stronger trust, build from source (`cargo install --git`) or verify against a maintainer GPG signature.

---

## Supply-chain Attack

An attack that targets the build or distribution pipeline rather than the application itself.

**Example (unpinned Actions):** A GitHub Action referenced as `@v6` can be compromised if the tag is moved to point to malicious code. Every subsequent CI run will execute that malicious version.

**Mitigation:** All Actions in `ci.yml` and `release.yml` are pinned to specific commit SHAs (e.g., `@de0fac2e4500dabe0009e67214ff5f5447ce83dd`). SHAs are immutable — a compromise would require a new SHA, which cannot be pushed to the existing pin.

---

## Least Privilege

The principle that every component should have only the permissions it absolutely needs.

**Example:** The `release.yml` workflow grants `contents: write` only to the job that creates the GitHub Release (`host`), not to every job. The `ci.yml` grants only `contents: read`.

**Why:** A compromised build step (e.g., a malicious dependency during compilation) cannot push to the repository or modify the release.

---

## Path Traversal

An attack where user-supplied paths escape the intended directory using `..` sequences or symlinks.

**Example:** `acari --scan-path ../../etc --clean --yes` would attempt to delete `/etc` files.

**Mitigation:** `prepare_targets()` in `commands.rs` filters all user-supplied paths through `is_safe_path()`, which rejects:
- Empty paths
- Paths containing `..`
- Exact top-level system directories (`/etc`, `/var`, `/sys`, `/proc`, `/dev`, `/boot`, `/bin`, `/sbin`, `/lib`, `/lib64`)

Additionally, `TargetConfig::add()` applies the same validation when persisting custom targets.

---

## Canonicalize

Resolving a path to its absolute, symlink-free form.

**Example:** `canonicalize("/home/user/foo/../bar")` returns `/home/user/bar`.

**Use in acari:** The `safe_canonicalize()` function in `cleaner.rs` resolves an entry's real path and checks that it starts with the target's canonical root. This prevents symlink-escap attacks during deletion.

---

## Restrictive Permissions (`0o600`)

File permissions that grant read+write only to the file owner, denying all access to group and others.

**Why for config:** `config.toml` may contain custom target paths that the user considers private (e.g., mount points of encrypted volumes). Default `0o644` would make them readable by any local user.

**Why for history:** `history.log` records cleaned targets and sizes. While not highly sensitive, `0o600` prevents information leakage on multi-user systems.

**Implementation:** `set_restrictive_permissions()` sets mode `0o600` via `chmod` after every write to config and history files.

---

## Log Rotation

Truncating or archiving a log file when it exceeds a size threshold.

**Why:** Acari's `history.log` appends an entry for every clean operation. Without rotation, it could grow unbounded over years of use, wasting disk space and potentially causing a mild DoS.

**Implementation:** `maybe_rotate()` in `history.rs` renames `history.log` to `history.log.old` when the file exceeds 100 KB, then starts a fresh file. The old log is kept for reference but won't grow further.

---

## Saturating Arithmetic

Arithmetic that clamps to the type's maximum/minimum value instead of wrapping around (overflow) or crashing (panic).

**Why:** Cache sizes can be very large (terabytes). Overflowing a `u64` counter would silently report a small or negative size. Panicking would crash the scanner.

**Implementation:** All byte/file counters use `.saturating_add()` (e.g., `scanner.rs:64-65`). The timestamp converter clamps to `i64::MAX` if the input exceeds it.

---

## Unbounded Channel

An asynchronous channel (`mpsc::unbounded_channel`) with no capacity limit.

**Why:** The scanner sends progress events to the TUI via an unbounded channel. If the TUI falls behind (e.g., slow rendering), messages accumulate in memory. In practice this requires thousands of outstanding events to be noticeable, and the scan is usually faster than the UI anyway.

**Risk level:** Low. The receiver is never dropped during a scan, and the channel is replaced on rescan.
