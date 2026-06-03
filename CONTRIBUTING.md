# Contributing

## Commit Convention

This project follows [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>: <description>

feat: add --exclude flag
fix: handle empty config path  
ci: bump Rust toolchain
docs: add security glossary
style: cargo fmt on all files
refactor: extract validate_path helper
test: add exclude pattern unit tests
chore: update dependencies
perf: reduce allocations in scanner
```

Types: `feat`, `fix`, `ci`, `docs`, `style`, `refactor`, `test`, `chore`, `perf`.

Breaking changes: append `!` after the type/scope (e.g., `feat!: remove --dry-run`).

## PR Rules

### Rebase merge only

All PRs must be **rebased** onto the target branch before merging. Merge commits are not allowed.

```bash
git fetch origin
git rebase origin/main
git push --force-with-lease
```

### No cherry-pick

Cherry-picking commits between branches is not allowed. If a fix is needed on a release branch, the fix must go through `main` first, then the release branch is rebased or the PR is re-targeted.

### One commit per PR

During development you can create as many atomic commits as you like in your branch.
Before opening a PR for review, rebase + squash them into a **single commit** describing the whole change.

```bash
git rebase -i origin/main
# squash everything into one commit
# write a meaningful commit message (see convention above)
git push --force-with-lease   # only on your own branch
```

`push --force-with-lease` is only allowed on your own feature/fix branches, never on `main` or shared branches.

If you need to address review feedback, amend the existing commit:

```bash
git add -A
git commit --amend --no-edit   # keep the same message
git push --force-with-lease    # only on your own branch
```

## AI Usage

AI-assisted contributions are allowed under these conditions:

- You **understand what the code does**. AI is a tool, not a replacement for understanding.
- You review every line generated before committing.
- You are responsible for correctness, security, and style.
- Obvious AI-generated code that is not reviewed will be rejected.

## Branch Naming

All branches (except `main` and release branches) must follow this pattern:

```
<type>/issue-<number>-<description>
```

| Part | Allowed values |
|------|---------------|
| `type` | `feature`, `bugfix` |
| `number` | Issue number from GitHub Issues |
| `description` | Lowercase, kebab-case, brief |

**Examples:**

```
feature/issue-42-add-exclude-flag
bugfix/issue-7-fix-path-traversal
```

**Release branches** are created by the `Prepare Release` workflow:

```
release/v0.2.0
```

## Before Opening a PR

1. Run `cargo fmt --all --check`
2. Run `cargo clippy --all-targets --all-features -- -D warnings`
3. Run `cargo test --all-targets`
4. If changing behavior, add or update tests
5. If adding a feature, consider if it needs documentation

## Security

See [SECURITY.md](SECURITY.md) for reporting vulnerabilities.
