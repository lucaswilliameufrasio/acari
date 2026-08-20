# ADR 003: Dedicated Rayon Pool for Directory Traversal

- **Status:** Accepted
- **Date:** August 20, 2026
- **Decision makers:** Lucas Eufrasio
- **References:**
  - [jwalk `Parallelism` docs](https://docs.rs/jwalk/latest/jwalk/enum.Parallelism.html)
  - [What We Learned Building a Rust Runtime for TypeScript — Encore](https://encore.dev/blog/rust-runtime)
  - [Untangling Tokio and Rayon in production — PostHog](https://posthog.com/blog/untangling-rayon-and-tokio)

## Context

Acari scans many cache and build directories to estimate reclaimable disk space.
Traversal uses `jwalk`, which can run on:

- `Parallelism::Serial` — the calling thread only,
- `Parallelism::RayonDefaultPool` — the rayon **global** thread pool,
- `Parallelism::RayonNewPool(n)` / `RayonExistingPool { pool, .. }` — a pool owned
  or provided by the caller.

Originally the scanner used `RayonDefaultPool` with a 1s `busy_timeout`, so every
concurrent directory walk submitted work to the single shared rayon global pool.
During a full scan the application runs up to four targets concurrently
(`std::thread::scope`), each walking deep directories of hundreds of thousands of
files.

Directory traversal is **I/O-bound** (`read_dir` + `metadata` syscalls), not
CPU-bound. Running it on the rayon global pool is the anti-pattern documented by
industry post-mortems:

- PostHog traced multi-second latency spikes to rayon work being submitted from
  I/O workers, saturating the shared pool.
- Encore moved all I/O-heavy infrastructure off rayon-style CPU pools onto a
  dedicated multi-threaded runtime.

Worse, `jwalk`'s `RayonDefaultPool` carries a **deadlock guard**: if the shared
pool fails to respond within `busy_timeout`, jwalk **aborts the iteration** with a
`ThreadpoolBusy` error. The scanner swallowed every walker error
(`Err(_) => continue`), so a saturated pool would silently **under-count** scanned
bytes and files — a correctness bug, not just a performance one.

## Decision

Use a **dedicated rayon `ThreadPool` per scan**, passed to each target walk via
`Parallelism::RayonExistingPool { pool, busy_timeout: None }`.

- The pool is created once per `start_background_scan` and shared by all targets
  in that scan. It is sized by I/O priority:
  - `High` → `available_parallelism()` threads,
  - `Normal` → half the cores (min 1),
  - `Low` → 1 thread (serial, respects `ionice` on Linux).
- Concurrency **between** targets remains bounded by `chunk_size` using dedicated
  OS threads via `std::thread::scope` (safe for I/O-bound work).
- `busy_timeout: None` disables jwalk's deadlock guard, which is safe because the
  pool is dedicated to this scan and never contended by unrelated work.
- Walker errors are skipped with an explicit best-effort comment instead of the
  previous duplicated `continue` branches.

## Consequences

### Positive

- **Correctness:** no silent under-counting from the shared-pool deadlock guard.
- **Performance:** measured on a full scan (macOS, 8 cores): ~22.9s vs ~27s with
  the original rayon global pool, and vs ~41s with serial internal walks. The
  dedicated pool is both faster and safer than the previous approach.
- **Resource budgeting:** pool size now follows I/O priority, and the number of
  concurrent walks is bounded.

### Negative / Trade-offs

- Each scan creates a fresh rayon pool (small one-time cost).
- `busy_timeout: None` relies on the pool being dedicated; if a future change
  routes unrelated work onto the same pool, the deadlock guard would need to be
  re-enabled.

### Risks

- On very low-latency NVMe systems the I/O subsystem itself (not CPU) can become
  the bottleneck; scaling target concurrency beyond a few threads may not help
  and can add syscall overhead. `chunk_size` keeps this bounded.

## Alternatives Considered

- **Keep `RayonDefaultPool`:** fastest conceptually but rejected because the
  shared pool can saturate and cause jwalk to abort silently.
- **Fully serial walks + OS threads only between targets:** correct and
  deterministic, but slower (~41s) because large single directories lose
  intra-directory parallelism.
- **`RayonNewPool` per walk:** correct but pays pool-construction cost per target
  instead of once per scan.
