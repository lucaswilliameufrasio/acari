# Dependency upgrade audit

Snapshot: 2026-11-05. This records a point-in-time review, not a guarantee
against future advisories or proof that every check occurred before the upgrade.
The baseline is `HEAD` before the uncommitted changes, not an earlier release.

## Direct Rust dependencies

The crates.io API `max_stable_version` was compared with the resolved direct
versions in `Cargo.lock`. Every direct application and development dependency
was at its latest stable release: anyhow 1.0.104, assert_cmd 2.2.2, clap 4.6.7,
crossterm 0.29.0, dirs 7.0.0, eframe 0.37.0, gpui-kit 0.9.0,
jwalk 0.9.0, libc 0.2.191, predicates 3.1.4, ratatui 0.30.2, rayon 1.12.0,
rfd 0.17.2, serde 1.0.229, serde_json 1.0.154, tempfile 3.27.0,
thiserror 2.0.21, tokio 1.54.1 and toml 1.2.0+spec-1.1.0. Multiple versions
of some crates remain in the lockfile because transitive requirements differ.
The release tools cargo-dist 0.34.0 and cargo-about 0.9.3 also matched the API.

## Breaking version upgrades

| Direct dependency | Baseline requirement | New resolved version | Evidence and limitations |
| --- | --- | --- | --- |
| dirs | 6.0 | 7.0.0 | Current and baseline lockfiles have no reported vulnerabilities. |
| toml | 0.8 | 1.2.0+spec-1.1.0 | Current and baseline lockfiles have no reported vulnerabilities. |
| eframe | 0.31 | 0.37.0 | Pre-1.0 compatibility boundary; both lockfiles have no reported vulnerabilities. |
| jwalk | 0.8 | 0.9.0 | Pre-1.0 compatibility boundary; both lockfiles have no reported vulnerabilities. |
| ratatui | 0.29 | 0.30.2 | Pre-1.0 compatibility boundary; both lockfiles have no reported vulnerabilities. |
| rfd | 0.15 | 0.17.2 | Pre-1.0 compatibility boundary; both lockfiles have no reported vulnerabilities. |

The optional gpui-kit 0.9.0 is new rather than an upgrade. Its evaluation was
audited before it was added, according to the session record; it introduced
additional unmaintained-dependency warnings. The exact pre-upgrade audit for
each other candidate was not preserved, so these retrospective checks must not
be described as proof of a pre-upgrade check.

## RustSec results and remaining scope

`cargo audit --json` refreshed RustSec to commit
`26fd2d43f7280a1fb925a3743ec463017afe7119` (database timestamp
2026-11-05T09:50:49+01:00): current application lockfile: **0 vulnerabilities**,
8 unmaintained warnings. The warnings are RUSTSEC-2024-0384 (instant),
RUSTSEC-2024-0436 (paste), RUSTSEC-2025-0134 (rustls-pemfile),
RUSTSEC-2026-0206 (rustybuzz), RUSTSEC-2026-0192 (ttf-parser),
RUSTSEC-2026-0371 (zbus_names), RUSTSEC-2026-0370 (zvariant), and
RUSTSEC-2026-0372 (zvariant_derive). These are maintenance warnings, not
vulnerability findings. The baseline audit also reported unsound warnings
RUSTSEC-2026-0002 and RUSTSEC-2026-0253 for lru 0.12.5; neither appears in
the current audit. Both tool lockfiles report zero vulnerabilities and warnings.

The Rust crate inventory covers direct crates and the two release tools, not
all transitive crates. RustSec only covers its own advisory database; a clean
result is not proof that no vulnerability exists. CI tooling is reviewed
separately below.

## CI and release tooling follow-up

Checked 2026-11-05 against the [official Rust stable manifest](https://static.rust-lang.org/dist/channel-rust-stable.toml)
and each linked upstream GitHub repository's latest release and published
repository security advisories. These are point-in-time results; empty upstream
advisory lists are not proof that no vulnerability exists.

| Tool or Action | Current references | Latest stable / finding |
| --- | --- | --- |
| Rust | CI and release pin 1.99.1; Cururu uses floating stable; prepare-release uses runner default | Official stable manifest reports 1.99.1. Floating/default toolchains are not reproducibly pinned. |
| [actions/checkout](https://github.com/actions/checkout/releases) | CI/release/prepare-release and Cururu workflows v7.0.1 SHA | v7.0.1; hosted-runner compatibility remains unverified. |
| [actions/upload-artifact](https://github.com/actions/upload-artifact/releases) | release and Cururu v7.0.2 SHA | Latest release v7.0.2. |
| [actions/download-artifact](https://github.com/actions/download-artifact/releases) | release and Cururu v8.0.2 SHA | Latest release v8.0.2, outside the affected range of [GHSA-cxww-7g56-2vh6](https://github.com/actions/download-artifact/security/advisories/GHSA-cxww-7g56-2vh6) (>=4.0.0, <4.1.3). |
| [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain) | CI/release master commit SHA; Cururu floating stable | Latest GitHub release is v1, but this action documents branch-based toolchains; release tag alone does not establish equivalence with the pinned master commit. |
| [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache/releases) | v2.9.3 SHA | Latest release v2.9.3. |
| [taiki-e/install-action](https://github.com/taiki-e/install-action/releases) | CI and prepare-release v2.90.1 SHA | Latest release v2.90.1. |
| [cururu](https://github.com/lucaswilliameufrasio/cururu/releases) | Cururu workflow pinned to a commit after v4.9.0, referencing its OCI image by digest | Latest release v4.9.0; registry manifest digest and local hash match the digest in the pinned Action. |
| [git-cliff](https://github.com/orhun/git-cliff/releases) | prepare-release builds 2.14.2 from source without default features | Latest release v2.14.2; see the reviewed residual advisories below. |
| [cargo-nextest](https://github.com/nextest-rs/nextest/releases) | CI explicitly selects 0.9.151 | Latest release 0.9.151. [GHSA-vc25-qg58-w57c](https://github.com/nextest-rs/nextest/security/advisories/GHSA-vc25-qg58-w57c) affects >=0.9.125 and <0.9.150 when rerunning portable recordings; fixed in 0.9.150. The workflow does not use that operation. |

The repository advisory APIs returned no published repository advisories for
checkout, upload-artifact, rust-toolchain, rust-cache, install-action, cururu,
or git-cliff; download-artifact and nextest reported the advisories above.
This does not include every GitHub-wide advisory or transitive dependency of
the prebuilt tools. CI's OS images, Python and prepare-release's default Cargo
are runner-provided rather than version-pinned. Remote runner execution and
compatibility of newer action majors remain unverified. In particular, the
Cururu workflow combines `pull_request_target`, PR-head checkout and a review
job with write permissions and a secret; its trust boundary needs separate
security review before changing behavior.

On 2026-11-05, Cururu's upload/download Actions were updated to their latest
stable upstream SHAs after reviewing repository advisories. The download v8
README demonstrates compatibility with upload v7; execution on GitHub remains
unverified. Rust-toolchain and the pre-existing checkout v4.4.0 and Cururu v4
commits were pinned without changing their currently resolved implementations.
Checkout v7 is deliberately deferred: its default fork-checkout protection
would block this workflow, and bypassing it would preserve an unsafe trust
boundary. The analyzer job checks out PR-controlled code and runs Cargo under
`pull_request_target`, where build scripts can execute; its output artifact is
then consumed by a separate job with write permissions and an LLM secret. This
is a source-level risk, not a demonstrated exploit. `persist-credentials:
false` and job separation alone do not establish trustworthy analyzer output.
Moving untrusted builds to an appropriately isolated `pull_request` workflow,
and securely binding sanitized results to the reviewed head before privileged
publication, requires a separately tested workflow design. The issue-comment
path previously lacked head identity verification. The privileged review job now checks out
the event's trusted base/default-branch commit (`github.sha`) with checkout
v7, rather than the PR head, and supplies the event's head SHA to Cururu for
`pull_request_target`. Issue comments are restricted to PRs and resolve the
current head via the GitHub API, failing closed on invalid values or API errors.
Cururu checks the expected head on publication.

The local changes now move Cargo analysis to a separate `pull_request`
workflow with read-only contents permissions, no declared secrets, no persisted
checkout credentials, and checkout v7. The privileged review no longer runs
the analyzer or downloads its files. It builds a minimal manifest from GitHub
API metadata, not from PR artifacts; raw analyzer logs are not delivered to
Cururu. `check_runs` remains enabled, but whether GitHub associates these
workflow checks with the exact head SHA queried by Cururu, and whether they are
available before review, has not been verified. Checks are untrusted evidence,
not proof of a safe PR. A GitHub-hosted runner is not by itself proof of a
network or filesystem sandbox, and same-repository PR permissions and repository
settings still matter. The analyzer only runs on PR events, not issue comments.
The Cururu release's Action uses a mutable image tag; the pinned Action commit
after that release instead uses the same image by digest. Do not enable
`allow-unsafe-pr-checkout` to bypass v7's protection. Hosted-runner tests have
not been run.

On 2026-11-05, the upstream release API listed v4.9.0 and no published
repository security advisories. Its annotated Git tag resolves to commit
`9a546f7242706bfe8a1644a14914998af092c622` (the Git tag's signature was not
verified). The currently pinned Action commit is two commits ahead; the
comparison reports only `action.yml` changed. Its image digest is
`sha256:0ad52d815d64e693b13a91fe09f8e375393a3fd80f371e72a241031a1b9d82c7`.
The GHCR v4.9.0 manifest reported that digest, which also matched the SHA-256
of the returned manifest bytes. This verifies tag-to-manifest identity at
lookup time, not image build provenance, transitive vulnerability status, or
future tag immutability.

The pinned Cururu implementation fetches check-run **annotations** for the
expected PR head, not arbitrary check conclusions. GitHub documents that
`pull_request` uses a merge-branch `GITHUB_SHA`; explicitly checking out the
head does not by itself establish which commit receives a check run. A read-only
inspection of historical PR 37 found 12 checks on its head and 15 on its merge
commit, but those are not a run of the newly added analysis workflow and do not
prove its association, annotations, or timing. The new workflow remains
unpublished and a live end-to-end association test is pending.

A later read-only API check of that closed, non-fork PR found one historical
`ci.yml` pull-request run whose head SHA, workflow ID and both repository IDs
matched, but `pull_requests` was empty in both the workflow-run list and the
individual run response. The current collector deliberately treats this shape
as `not_run`, even if its conclusion were failure; a local fixture covers the
same-repository, second-attempt shape. This observation is not evidence about
open PRs or the unpublished analysis workflow. It does demonstrate that SHA
and repository equality alone cannot be assumed to yield PR-linked evidence;
restoring useful status in this case needs a separately verified identity
strategy, not a silent relaxation of the current check.

The authenticated check-suite endpoint for that same historical run was also
inspected: its head SHA matched the run and it had a head branch, but its
`pull_requests` list was empty as well. The run listed seven jobs with check-run
URLs; their existence does not establish which PR triggered the run. Looking up
the check suite alone therefore does not recover the missing association in this
example. A branch name or head SHA can be shared by multiple PRs, and querying
the PRs currently associated with a commit does not prove which PR triggered a
historical attempt. A possible future design is to persist an authenticated,
PR-specific GitHub event association at execution time and bind it to the exact
run and attempt, without accepting claims from PR-executed code; its trust,
fork permissions, reruns and delivery races still require validation. Until
then, empty linkage remains `not_run`; this can omit a genuine failure.

A regression fixture also showed that a run linked to multiple PR numbers was
previously attributed to each matching PR. The collector now treats multiple
links as ambiguous, including when a newer ambiguous run follows an older
failure. This deliberately favors `not_run` over a possibly incorrect failure;
it does not prove that a single linked PR identifies the original triggering
event in every GitHub API response.

GitHub's [event documentation](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)
rules out another apparent shortcut: `check_run` and `check_suite` events do
not trigger Actions workflows when the suite was created by GitHub Actions (or
its head SHA is associated with Actions). A privileged listener on those
events cannot therefore be assumed to capture this analyzer's identity. The
same documentation describes the `pull_request` ref as
`refs/pull/<number>/merge`, but a ref observed inside a PR-controlled workflow
or supplied through its artifacts is not independently authenticated evidence
of the run's triggering PR. The documentation alone does not establish an
API-accessible binding from that ref to a particular run attempt. No listener
or fallback was enabled on that basis; a trusted event receiver and its
run/attempt binding remain an unverified design option, not an implementation.

Source inspection of the pinned Cururu revision confirms `AnalysisConfig`
defaults `manifest` to `None`; omitting it does not cause a missing-file read.
With analysis enabled, no manifest, no SARIF paths and no returned annotations,
`load_evidence` reports `no_evidence`, not a passed analysis. Its GitHub adapter
fetches check runs for the supplied head SHA, skips successful runs, and reads
annotations from other runs; it does not turn a failed check with no annotations
into an analyzer failure. The current `check_run_names = []` does not restrict
which check's annotations can be considered. The two workflows execute
independently, so review can also finish before analysis. These are observed
source semantics, not evidence that the new workflow's checks are associated
with the expected SHA or that GitHub produces annotations for Cargo output.

`scripts/collect_cururu_analysis.py` queries the authenticated GitHub API in
the trusted review job, checks the PR head and repository identities before
and after lookup, and projects the newest matching workflow run into a minimal
manifest. No PR artifact or run text is used. Missing, ambiguous, pending,
cancelled, truncated or successful results become `not_run`; matching failures
become `failed`. Success is intentionally not asserted because a PR can alter
its workflow. API errors or changed PR identity fail the step closed. The
manifest directory must not already exist. `expected_head_sha` separately
guards Cururu publication, but there is still a race after lookup; the pinned
Cururu's check is responsible for rejecting a changed head. The query does not
wait or retry for analysis completion, and run-to-PR metadata may be absent,
especially for forks; these cases remain unknown. Matching metadata does not
authenticate the workflow contents or establish safe code. Local fixture tests
cover projection and identity changes; actual GitHub API behavior, timing and
end-to-end review are unverified. A read-only call through the production
`gh_api` adapter to historical PR 37 returned a 40-character head SHA and both
repository IDs; it did not exercise the new unpublished workflow-run endpoint.
Fork and incomplete-response fixtures exposed a stale-failure selection bug:
a newer run without PR linkage or a duplicate run ID could previously allow an
older failure to be selected. The collector now treats such lists as ambiguous
and returns `not_run`; the regression fixtures pass. This conservative behavior
may also omit a legitimate failure until complete linkage is available.
Additional POSIX process-level fixtures invoke the actual collector script and
`gh_api` adapter with a fake `gh` executable and synthetic token. They verify
the PR/workflow/run/PR request sequence, manifest projection, and that a changed
head on the second PR lookup fails without writing a manifest. These fixtures
do not run the Cururu container or establish GitHub-hosted runner behavior.

On 2026-11-05, before updating prepare-release, upstream release and repository
advisory APIs were checked: checkout v7.0.1 and install-action v2.90.1 were the
latest releases, their exact release tags resolve to the selected full SHAs,
and neither repository listed published security advisories. Checkout v7
requires runner v2.327.1 or newer and blocks fork PR checkouts under
`pull_request_target` by default; prepare-release is manually dispatched on
main, not a PR checkout. Actual hosted-runner execution remains unverified.

Correction: the previous characterization of CI's nextest as unpinned was
incorrect. The [install-action documentation](https://github.com/taiki-e/install-action/blob/v2.90.1/README.md)
says its pinned Action version also pins a tool selected without a version;
its [manifest](https://github.com/taiki-e/install-action/blob/v2.90.1/manifests/cargo-nextest.json)
selects 0.9.151. The workflow now spells out that same version. This uses
upstream prebuilt binaries, whose transitive dependencies have not been audited.

### git-cliff 2.14.2 investigation

On 2026-11-05, crates.io and the upstream release API identified 2.14.2 as
the latest stable version. Its published Cargo.lock was checked against the
local RustSec database: `cargo audit --file <published-lock> --no-fetch`
reported 23 vulnerability advisories, including RUSTSEC-2026-0418 in libz-sys
and RUSTSEC-2026-0414/0433 in pulldown-cmark. This is an all-lockfile scan,
including optional dependencies, not a statement that every advisory affects
the actual binary. The audit also could not check some yanked status entries
because the local index was incomplete. The dependency graph of the upstream
prebuilt binary is not established by the crate's published lockfile.

As an experiment, `cargo update` on an isolated copy of the published manifest
and lockfile reduced the audit to two vulnerabilities, RUSTSEC-2026-0194 and
RUSTSEC-2026-0195, both in quick-xml 0.26.0 through optional profiler/inferno.
`cargo tree --locked --no-default-features -i quick-xml@0.26.0` did not find
that package in the current host's selected graph; other targets and features
remain unverified. The candidate built on Linux with `cargo build --locked
--no-default-features`. A synthetic repository with a tagged baseline and a
feature commit produced identical 183-byte changelog output with local
git-cliff 2.13.1 and the 2.14.2 candidate using `cliff.toml`, `--bump` and
`--tag`. This is one compatibility fixture, not full release verification.
The updated lockfile still reports two unmaintained warnings and one unsound
warning, including memmap2 0.5.10, which appeared in the Linux build. The
former 2.13.1 prebuilt binary's dependency graph was not established.

#### Follow-up: memmap2 advisory reachability

RustSec [RUSTSEC-2026-0186](https://rustsec.org/advisories/RUSTSEC-2026-0186)
describes unchecked out-of-bounds offsets in range-based advice and flush
methods; versions before 0.9.11 are affected. The selected Linux graph is
`git-cliff -> git-cliff-core 2.14.2 -> cacache 13.1.0 -> memmap2 0.5.10`.
The core manifest explicitly enables cacache's `mmap` feature, and 13.1.0 is
the latest cacache release checked on 2026-11-05. A source search of cacache
13.1.0 found no calls to the advisory's range methods. Its asynchronous writer
calls `MmapMut::flush_async()` instead; memmap2 0.5.10 implements that call
with offset zero and the mapping's own length. Core's local changed-files
cache uses cacache's synchronous read and write functions. This narrows the
identified risk but is not proof of absence of other reachable unsafe paths,
and does not remove the RustSec warning. A patched compatible transitive
release was not identified. The earlier unmodified-manifest candidate lockfile
also contained the optional quick-xml findings and unmaintained `bincode`.

The earlier unmodified-manifest candidate was used for `cargo tree`. With
`--no-default-features --target all`, bincode 2.0.1 is selected directly by
git-cliff-core; bincode 1.3.3 is not selected. RustSec RUSTSEC-2025-0141
classifies bincode as unmaintained and lists no patched version; this is not
itself a vulnerability advisory. Replacing the selected dependency requires
an upstream change or a maintained fork. That earlier candidate's full lock
reported two quick-xml vulnerabilities, two bincode warnings and the memmap2
warning; the current transformed lock no longer includes quick-xml 0.26.

The 2.14.2 source uses bincode to serialize and deserialize the local
changed-files cache when path filtering is active or when a nonempty
`.git-blame-ignore-revs` file requires changed-file checks. The current
`cliff.toml` and release command do not set include/exclude paths. In a
synthetic repository without that ignore file, the installed release binary
created no changed-files cache with the release flags, while the same command
with `--include-path '**'` did create it. This fixture narrows the observed
default path only; it does not establish that every release environment lacks
an ignore file or cache, or that other bincode uses are impossible.

The prepare-release workflow now builds 2.14.2 from a crates.io archive whose
SHA-256 matches the crates.io API and `tools/locks/git-cliff-2.14.2.lock`, with
`--no-default-features` and Rust 1.99.1. The builder verifies the archive, then
applies the exact version-specific manifest transform and uses the reviewed
lockfile. A no-fetch audit of that checked-in lock now reports zero
vulnerabilities. The memmap2 and bincode warnings remain; this is a scoped risk
assessment, not a clean audit or a guarantee of safety. The exact builder
successfully installed a release-profile binary on Linux; a synthetic Git
repository verified generation with the expected version and feature. Hosted
runner compatibility and the complete release workflow remain unverified.
The builder's local fixture also verifies that an accepted archive's bundled
lockfile is replaced by the reviewed lock and that `--locked` and the requested
feature flags are passed to Cargo. The fixture substitutes archive download and
the install process; it is not a remote runner or production build test.

The optional profiler chain is constrained to `pprof 0.15.0 -> inferno 0.11`
and inferno 0.11.21 requires quick-xml 0.26. The newer inferno 0.12.8 requires
quick-xml 0.41, but pprof's 0.11 requirement cannot select that incompatible
minor release automatically. The installed tool uses `--no-default-features`
without the optional profiler. Eliminating the two full-lockfile advisories
therefore requires an upstream dependency update or a maintained patch/fork;
no advisory was suppressed.

Fresh upstream check on 2026-11-05: crates.io lists git-cliff 2.14.2, pprof
0.15.0, inferno 0.12.8 and cacache 13.1.0 as the latest stable versions.
At [git-cliff main `4f65432`](https://github.com/orhun/git-cliff/tree/4f65432ba95ad97147da655fac4071b4bbccb1db),
the optional profiler still selects pprof 0.15.0 and core still selects
cacache 13.1.0 and bincode 2.0.1. At [pprof master `01cff82`](https://github.com/tikv/pprof-rs/tree/01cff82dbe6fe110a707bf2b38d8ebb1d14a18f8),
the manifest still requires inferno 0.11. A local experiment on an isolated
copy, `cargo update -p inferno --precise 0.12.8`, failed with Cargo's
`failed to select a version for the requirement inferno = ^0.11`. No upstream
compatible update was identified; the experiment changed neither checked-in
lockfiles nor release dependencies. GitHub's latest-release endpoint for
pprof returned 404, so its stable-version comparison comes from crates.io.

An additional isolated experiment removed only the optional `pprof` dependency
from a copy of the published git-cliff 2.14.2 manifest, replaced its profiler
feature definitions so they no longer reference pprof, and generated a new
lockfile offline. `cargo audit --no-fetch` then reported **zero vulnerabilities**
and three remaining warnings (two unmaintained, one unsound); `cargo check
--locked --offline --no-default-features` passed on Linux. This is a modified
upstream manifest, not the original release: enabling its profiler would no
longer work as upstream intended. It is an isolated proof of feasibility, not
an audit of every possible feature or remote runner validation. The experiment
was subsequently adopted via the version-specific builder transformation below;
this trades an unchanged upstream package for a locally maintained variant.

A subsequent Linux end-to-end fixture independently transformed two source
copies with exact-match checks (refusing unexpected manifests) and generated
identical offline lockfiles, SHA-256
`4324122fc0d1fd7c26584a0f4916de9e6a4d953e7c134c35789cb02ddac35a3d`.
The resulting lockfile audit reported zero vulnerabilities and the same three
warnings. A release-profile, offline, locked, no-default-features build produced
git-cliff 2.14.2. In a synthetic tagged repository, its changelog output matched
the existing source-built 2.14.2 binary byte-for-byte both with the release
flags and with `--include-path '**'`; both contained the expected version and
feature. These observations cover two synthetic cases on Linux and lockfile
reproduction with the current local index, not all configurations, platforms,
or reproducible binaries. The transformation is now integrated into the builder;
profiler support is unavailable in the modified package. The integrated builder
successfully built and installed the release binary on Linux, and two synthetic
changelog cases matched the previously built original-manifest binary. Tests
ensure an unexpected, duplicated or already-transformed manifest is rejected.
Hosted runner and complete release workflow testing remain required.
