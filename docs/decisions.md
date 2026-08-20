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

## ADR 003: Dedicated Rayon Pool for Directory Traversal
- Date: August 20, 2026
- Context: Directory walks on the rayon global pool can saturate it and make jwalk abort silently, under-counting scan results.
- Decision: Use a dedicated rayon `ThreadPool` per scan (`RayonExistingPool`, `busy_timeout: None`), sized by I/O priority; keep concurrency between targets on OS threads via `std::thread::scope`.
- Consequences: Correct, deterministic results and faster scans (~22.9s vs ~27s); see `docs/adr/003-dedicated-rayon-pool-for-directory-traversal.md`.
