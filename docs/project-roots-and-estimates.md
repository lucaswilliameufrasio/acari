# Project roots and scan estimates

Project discovery is opt-in. An empty root list means **not scanned**, not a
measurement of zero project artifacts. CLI, TUI and desktop share the same
validated root configuration. Existing common development directories may be
suggested, but are never registered or scanned without an explicit action.

Root registration validates and canonicalizes an existing directory, rejects
system-wide/home roots and deduplicates aliases. Removing a root only changes
configuration; it never deletes the directory. The desktop can inspect each
configured root using its existing read-only disk analyzer.

The breakdown presents candidates, not guaranteed savings or an exact partition
of filesystem usage. Directory estimates use allocated blocks; command estimates
may overlap each other, refer to another filesystem, or include shared data.
Dangerous targets and incomplete scans must be visible, not silently counted as
safe reclaimable bytes. Custom project patterns and exclusions apply to the
breakdown as well as explicit project scans.

Automatically adding the home directory was rejected: it increases scan cost and
can classify unrelated directories by name. Suggestions require confirmation.

## Verification

Verified locally on Linux: format, Clippy with warnings denied, all-target/all-feature
check and build, rustdoc, installer syntax, and 258 tests with nextest. Tests cover
opt-in setup, persisted roots, aliases, invalid paths, discovery/scan errors,
custom patterns and exclusions, JSON overlap accounting, and UI empty/error states.
A PTY smoke check exercised the main TUI → project manager → suggested root →
Enter → persisted configuration → return/exit, preserving fixture files. Desktop
rendering was tested with egui without a native window; macOS/Windows and native
desktop interaction were not executed locally. No installed binary or user roots
were changed and no user data cleanup was performed.
