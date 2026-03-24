# Project Structure (TUI / CLI App)

This document defines the target architecture for the Rust TUI cleaner application in this repository.
This standard mirrors a boundary model adapted from backend services (`ui -> application -> domain <- infrastructure`) tailored to idiomatic Rust CLI/TUI applications.

## High-level layout

```txt
.
├── build.rs
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── ui/
│   ├── application/
│   ├── domain/
│   ├── infrastructure/
│   └── config/
├── bin/
│   └── headless_cleaner.rs
├── tests/
├── docs/
├── target/
├── Cargo.toml
├── Cargo.lock
└── README.md
```

## Layering and dependency rules

```txt
ui -> application -> domain
ui -> application -> infrastructure
infrastructure -> domain
```

Rules:
- `domain` remains pure Rust.
- `application` orchestrates background work and channels.
- `ui` handles terminal concerns only.
- `infrastructure` owns OS-specific behaviors.
