# Architecture Decision Records (ADR)

## ADR 001: Parallel Directory Traversal
- Date: March 11, 2026
- Context: Recursive `std::fs::read_dir` is slow for very deep dependency directories.
- Decision: Use `jwalk` for parallel traversal.
- Consequences: Faster scans; must tolerate permission errors without panics.

## ADR 002: Single Cross-Platform Codebase
- Date: March 11, 2026
- Context: Maintaining separate macOS/Linux binaries with duplicated UI logic is costly.
- Decision: Keep one codebase and isolate platform logic with `#[cfg(target_os = "...")]`.
- Consequences: Less duplication and cleaner maintenance; platform code must remain quarantined.
