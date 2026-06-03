# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| latest  | ✅ |

## Reporting a Vulnerability

Acari is a disk cleaner that operates on your filesystem. While we take precautions (path validation, `0o600` permissions, TOCTOU mitigation), no software is perfect.

If you find a security vulnerability:

1. **Do not** open a public GitHub Issue.
2. Send an email to **lucas@eufrasio.dev** with details.
3. Include:
   - Affected version / commit SHA
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

You should receive a response within 48 hours. If you don't, please follow up.

## Scope

The following are **in scope**:
- Path traversal via `--scan-path`, `--target`, or config targets
- Symlink / TOCTOU attacks during scan or clean
- Privilege escalation via the CLI
- CI/CD supply-chain attacks (unpinned actions, compromised dependencies)

The following are **out of scope**:
- Attacks requiring physical access
- Attacks requiring the attacker already has access to your user account
- Denial of service via extremely large directories (caches can be large by nature)

## Recognition

We will credit researchers who report valid vulnerabilities in the release notes,
unless they prefer to remain anonymous.

## Security features

See [docs/glossary.md](docs/glossary.md) for explanations of the security
mechanisms in the codebase (TOCTOU mitigation, path filtering, restrictive
permissions, etc).
