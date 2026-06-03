# Releasing

## Versioning Policy

- This project follows Semantic Versioning (`MAJOR.MINOR.PATCH`).
- Use tags in the format `vX.Y.Z`.
- Update `CHANGELOG.md` before creating a release tag.

## Changelog generation (git-cliff)

The changelog is auto-generated from git history using [git-cliff](https://git-cliff.org).

Commits must follow [Conventional Commits](https://www.conventionalcommits.org/) format:

```text
feat: add --exclude flag
fix: handle empty config path
ci: bump Rust toolchain
docs: add security glossary
```

Types: `feat`, `fix`, `ci`, `docs`, `style`, `refactor`, `test`, `chore`, `perf`.

---

## Prerequisites (cargo-dist)

Everything is already configured. [cargo-dist](https://github.com/axodotdev/cargo-dist) handles:

- Cross-platform builds (Linux x86_64/arm64, macOS x86_64/arm64, Windows x86_64)
- Tarballs/zips with SHA256 checksums
- Install scripts (shell + powershell)
- Homebrew formula (if enabled)
- GitHub Release creation with auto-generated changelog

**No GitHub configuration needed.** The workflow uses automatic `GITHUB_TOKEN`.

## Release Steps

1. Ensure branch is green (fmt, clippy, test).

2. Generate the changelog from git history:

```bash
# Preview unreleased changes
git cliff --unreleased

# Regenerate the full CHANGELOG.md (auto-detect version)
git cliff -o CHANGELOG.md
```

   Review and commit the updated `CHANGELOG.md`.

3. Create and push a release tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

4. The `Release` workflow in GitHub Actions will:
   - `plan`: calculate which artifacts to build
   - `build-local-artifacts`: compile for each target, generate tarballs/zips + checksums
   - `build-global-artifacts`: generate installers (shell + powershell)
   - `host`: create GitHub Release, upload artifacts, generate changelog
   - `announce`: placeholder for notifications

### Generated artifacts

For each tag, cargo-dist generates:

| File | Description |
|------|-------------|
| `acari-{tag}-{target}.tar.gz` | Packaged binaries (Unix) |
| `acari-{tag}-{target}.zip` | Packaged binaries (Windows) |
| `acari-{tag}-{target}.tar.gz.sha256` | Checksum |
| `acari-installer.sh` | Shell install script |
| `acari-installer.ps1` | PowerShell install script |

### Install via cargo-dist

```bash
# Latest release (shell)
curl -fsSL https://github.com/lucaswilliameufrasio/acari/releases/latest/download/acari-installer.sh | sh

# Specific version
curl -fsSL https://github.com/lucaswilliameufrasio/acari/releases/download/v0.2.0/acari-installer.sh | sh
```

## cargo-dist documentation

- Repository: https://github.com/axodotdev/cargo-dist
- Documentation: https://opensource.axo.dev/cargo-dist/
- Current config: `dist-workspace.toml`
- Installed version: 0.32.0

## Customization

To change targets, installers or other settings:

```bash
cargo dist init
```

Or edit `dist-workspace.toml` directly.

## Post-release

- Move completed entries from `[Unreleased]` into a new version section in `CHANGELOG.md`.
- Keep checksum validation instructions in `README.md` up to date.
