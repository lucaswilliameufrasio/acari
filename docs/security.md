# Security & Permissions

This tool is designed around least privilege and local-only processing.

## Principle of Least Privilege

The app runs in user space by default and scans paths the current user can access.

## Destructive Actions Policy

- Headless destructive cleanup requires explicit confirmation with `--yes`.
- `--clean --dry-run` is non-destructive and does not require `--yes`.
- In TUI mode, cleaning is explicit via manual target selection + Enter confirmation.

## Known Privilege Boundaries

- macOS SIP paths may produce `EACCES`; scanner skips them.
- Linux system paths (`/var/log/journal`, `/var/cache/apt/archives`) may require root.
- Cleanup may fail per target on permission-denied paths; these failures are reported in output as non-zero errors.

## Data Privacy

The scanner processes file metadata locally and does not send telemetry or scan data externally.

## Safe Usage Recommendations

- First run with `--headless --clean --dry-run` to review intended cleanup scope.
- Use `--scan-path` for constrained, explicit cleanup boundaries during validation.
- Use `--clean --yes` only after confirming targets and expected reclaimable bytes.
