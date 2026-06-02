# Releasing

## Versioning Policy

- This project follows Semantic Versioning (`MAJOR.MINOR.PATCH`).
- Use tags in the format `vX.Y.Z`.
- Update `CHANGELOG.md` before creating a release tag.

## Pré-requisitos (cargo-dist)

Já está tudo configurado. O [cargo-dist](https://github.com/axodotdev/cargo-dist) gerencia:

- Build cross-platform (Linux x86_64/arm64, macOS x86_64/arm64, Windows x86_64)
- Geração de tarballs/zip com checksums SHA256
- Scripts de instalação (shell + powershell)
- Homebrew formula (se habilitado)
- Criação da GitHub Release com changelog

**Nada precisa ser configurado no GitHub.** O workflow usa `GITHUB_TOKEN` automático.

## Release Steps

1. Ensure branch is green (fmt, clippy, test).
2. Update `CHANGELOG.md` in the `[Unreleased]` section.
3. Create and push a release tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

4. O workflow `Release` no GitHub Actions vai:
   - `plan`: calcular quais artifacts construir
   - `build-local-artifacts`: compilar para cada target, gerar tarballs/zip + checksums
   - `build-global-artifacts`: gerar installers (shell + powershell)
   - `host`: criar a GitHub Release, fazer upload dos artifacts, criar o changelog automaticamente
   - `announce`: notificar (placeholder)

### Artefatos gerados

Para cada tag, o cargo-dist gera:

| Arquivo | Descrição |
|---------|-----------|
| `acari-{tag}-{target}.tar.gz` | Binários compactados (Unix) |
| `acari-{tag}-{target}.zip` | Binários compactados (Windows) |
| `acari-{tag}-{target}.tar.gz.sha256` | Checksum |
| `acari-installer.sh` | Script de instalação via shell |
| `acari-installer.ps1` | Script de instalação via PowerShell |

### Instalação via cargo-dist

```bash
# Última release (shell)
curl -fsSL https://github.com/lucaswilliameufrasio/acari/releases/latest/download/acari-installer.sh | sh

# Versão específica
curl -fsSL https://github.com/lucaswilliameufrasio/acari/releases/download/v0.2.0/acari-installer.sh | sh
```

## Documentação do cargo-dist

- Repositório: https://github.com/axodotdev/cargo-dist
- Documentação: https://opensource.axo.dev/cargo-dist/
- Config atual: `dist-workspace.toml`
- Versão instalada: 0.32.0

## Customização

Para alterar targets, installers ou outras configs:

```bash
cargo dist init
```

Ou edite `dist-workspace.toml` diretamente.

## Post-release

- Move completed entries from `[Unreleased]` into a new version section in `CHANGELOG.md`.
- Keep checksum validation instructions in `README.md` up to date.
