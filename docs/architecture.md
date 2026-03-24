# Architecture

This project follows a layered architecture adapted for Text User Interfaces.

## Layer Flow
`ui -> application -> domain <- infrastructure`

- UI (`src/ui`): Consumes `AppEvent` messages via `mpsc` and renders output.
- Application (`src/application`): Spawns workers and coordinates flow via `scanner`, `cleaner`, `headless`, and `commands`.
- Domain (`src/domain`): Core models (`CleanTarget`, `ScanResult`, `AppEvent`) and target composition (`targets`, `custom_targets`).
- Infrastructure (`src/infrastructure`): Filesystem scanning and OS-specific modules.

## Concurrency Model

The UI loop must not block. I/O operations execute in background worker threads and emit updates using `mpsc::channel`.
