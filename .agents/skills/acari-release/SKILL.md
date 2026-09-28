---
name: acari-release
description: >-
  Use whenever the user asks to release Acari, bump its version, create a tag,
  or publish a GitHub release. Determine the next Semantic Version and follow
  this repository's Prepare Release PR, merge, tag, and cargo-dist publication
  workflow end to end.
---

# Acari release workflow

Follow `docs/releasing.md` as the repository-specific source of truth. The standard flow is automated: dispatch **Prepare Release**, review and merge its generated release PR, then push the matching version tag to start cargo-dist.

## Determine the version

1. Inspect `git status`, the current branch, `Cargo.toml`, `CHANGELOG.md`, the latest GitHub Release/tag, and any open release PRs. Do not infer the next version from a possibly stale local checkout.
2. Compare changes since the latest stable tag using Semantic Versioning:
   - breaking API/behavior change: increment MAJOR and reset MINOR/PATCH;
   - backward-compatible feature: increment MINOR and reset PATCH;
   - bug fix or maintenance-only release: increment PATCH.
3. Treat release-candidate tags (`-rc.N`) as pre-releases, not the latest stable version. Normalize the workflow input to `X.Y.Z` without a leading `v`.
4. State the selected version and why it fits SemVer before dispatching if the user did not supply a version.

## Prepare and review

1. Confirm `main` is current and the worktree is clean. Do not include unrelated local changes in the release.
2. Run the repository's `Prepare Release` workflow against `main` with the chosen version, for example:

   ```sh
   gh workflow run "Prepare Release" --ref main --field version=0.8.3
   ```

3. Wait for the workflow to finish and inspect its generated release PR. Confirm that:
   - the PR targets `main` and has the expected `release/vX.Y.Z-rc.N` head;
   - `Cargo.toml` is bumped to exactly `X.Y.Z`;
   - the changelog contains the intended commits under that version;
   - `Cargo.lock` has the correct Acari package version. Review any dependency version changes from lockfile regeneration instead of assuming they are part of the release feature.
4. Run the project's format, Clippy, build, and full test checks against the release PR. Wait for applicable CI and automated review checks. Resolve actionable findings before merging.
5. Merge the release PR only as part of the user's release request and after checks are green. Do not tag the release-candidate branch or an unmerged commit.

## Tag and publish

1. Update local `main` to the merged release commit. Verify the package version and that the exact `vX.Y.Z` tag does not already exist; never move or overwrite a tag.
2. Create and push the tag on the merged `main` commit:

   ```sh
   git tag v0.8.3
   git push origin v0.8.3
   ```

3. Wait for the `Release` GitHub Actions workflow. Confirm plan, all platform artifact builds, global installer build, and host/publish jobs succeed.
4. Verify the GitHub Release exists, is not a draft or pre-release, has the expected tag/version, and includes platform archives, checksums, installers, and the dist manifest.
5. Report the SemVer rationale, release PR, tag, Release URL, artifact/workflow status, and the final worktree state. If any release job fails or approval is required, report the exact blocker and stop before claiming publication succeeded.

## Safety and scope

- Creating a version-preparation PR and publishing a release are consequential external actions; perform them only when the user asked to release or explicitly approved those steps.
- Use GitHub (`gh`) for PRs, Actions runs, and releases. Use the documented workflow instead of manually editing the changelog/version or manually assembling release assets.
- Do not push or merge unrelated changes, rewrite history, or force-update tags.
