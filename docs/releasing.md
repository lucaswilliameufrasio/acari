# Releasing

## Versioning Policy

- This project follows Semantic Versioning (`MAJOR.MINOR.PATCH`).
- Use tags in the format `vX.Y.Z`.
- Update `CHANGELOG.md` before creating a release tag.

## Release Steps

1. Ensure branch is green (`fmt`, `clippy -D warnings`, `test`).
2. Update `CHANGELOG.md` in the `[Unreleased]` section.
3. Create and push a release tag:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

4. GitHub Actions workflow `Release Binaries` will:
- build release binaries for Linux/macOS/Windows,
- package artifacts,
- generate `.sha256` files,
- upload artifacts and checksums to the GitHub Release.

## Post-release

- Move completed entries from `[Unreleased]` into a new version section in `CHANGELOG.md`.
- Keep checksum validation instructions in `README.md` up to date.
