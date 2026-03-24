# Environment Variables

The current implementation does not require custom environment variables for core behavior.

| Variable | Description | Default |
| :--- | :--- | :--- |
| `RUST_LOG` | Controls log verbosity if logging is enabled by runtime instrumentation. | unset |

## Runtime Flags (Preferred Configuration Surface)

Use CLI flags instead of environment variables for behavior control:

- `--headless`: run without TUI.
- `--clean`: clean immediately after scan in headless mode.
- `--dry-run`: simulate cleanup without deleting files (requires `--clean`).
- `--yes`: confirm destructive cleanup (required with `--clean` when not using `--dry-run`).
- `--scan-path <PATH>`: append custom ad-hoc paths to scan.
- `--target <NAME>`: filter predefined targets by name.
