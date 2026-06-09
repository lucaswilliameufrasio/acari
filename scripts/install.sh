#!/usr/bin/env sh
set -eu
#
# acari installer (TOFU — Trust On First Use)
#
# The binary archive and its SHA256 checksum are both served from the same
# GitHub Releases source. This provides transport security (HTTPS) and
# integrity verification against accidental corruption, but does NOT protect
# against a compromised release. For stronger trust anchors, consider:
#   - Verifying the release against a maintainer GPG signature
#   - Building from source: cargo install --git https://github.com/lucaswilliameufrasio/acari

REPO=""
TAG="latest"
BIN_DIR="${HOME}/.local/bin"
FORCE=""

usage() {
  cat <<USAGE
acari installer

Usage:
  install.sh --repo <owner/repo> [--tag <vX.Y.Z|latest>] [--bin-dir <path>]

Options:
  --repo      GitHub repository in owner/repo format (required)
  --tag       Release tag (default: latest)
  --bin-dir   Install directory (default: ~/.local/bin)
  --force     Remove old installations without prompting
  -h, --help  Show this help
USAGE
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      REPO="$2"
      shift 2
      ;;
    --tag)
      TAG="$2"
      shift 2
      ;;
    --bin-dir)
      BIN_DIR="$2"
      shift 2
      ;;
    --force)
      FORCE="yes"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [ -z "$REPO" ]; then
  echo "--repo is required" >&2
  usage >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required" >&2
  exit 1
fi

if ! command -v tar >/dev/null 2>&1; then
  echo "tar is required" >&2
  exit 1
fi

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    case "$ARCH" in
      x86_64|amd64) TARGET="x86_64-unknown-linux-gnu" ;;
      *)
        echo "Unsupported Linux architecture: $ARCH" >&2
        exit 1
        ;;
    esac
    ;;
  Darwin)
    case "$ARCH" in
      arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
      x86_64|amd64) TARGET="x86_64-apple-darwin" ;;
      *)
        echo "Unsupported macOS architecture: $ARCH" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "Unsupported OS for this installer: $OS" >&2
    echo "Use release artifacts manually for your platform." >&2
    exit 1
    ;;
esac

if [ "$TAG" = "latest" ]; then
  RELEASE_JSON="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")"
  TAG="$(printf '%s' "$RELEASE_JSON" | tr -d '\n' | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
  if [ -z "$TAG" ]; then
    echo "Failed to resolve latest release tag for $REPO" >&2
    exit 1
  fi
fi

ASSET="acari-${TAG}-${TARGET}.tar.gz"
BASE_URL="https://github.com/${REPO}/releases/download/${TAG}"
ASSET_URL="${BASE_URL}/${ASSET}"
CHECKSUM_URL="${BASE_URL}/${ASSET}.sha256"

TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t acari-install)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

ASSET_FILE="${TMP_DIR}/${ASSET}"
CHECKSUM_FILE="${TMP_DIR}/${ASSET}.sha256"

echo "Downloading ${ASSET_URL}"
curl -fsSL "$ASSET_URL" -o "$ASSET_FILE"

echo "Downloading ${CHECKSUM_URL}"
curl -fsSL "$CHECKSUM_URL" -o "$CHECKSUM_FILE"

EXPECTED_HASH="$(awk '{print $1}' "$CHECKSUM_FILE")"
if [ -z "$EXPECTED_HASH" ]; then
  echo "Invalid checksum file format" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  ACTUAL_HASH="$(sha256sum "$ASSET_FILE" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  ACTUAL_HASH="$(shasum -a 256 "$ASSET_FILE" | awk '{print $1}')"
else
  echo "Neither sha256sum nor shasum is available for checksum verification" >&2
  exit 1
fi

if [ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]; then
  echo "Checksum mismatch for ${ASSET}" >&2
  echo "Expected: $EXPECTED_HASH" >&2
  echo "Actual:   $ACTUAL_HASH" >&2
  exit 1
fi

echo "Checksum verified"

tar -xzf "$ASSET_FILE" -C "$TMP_DIR"

mkdir -p "$BIN_DIR"
install_bin() {
  src="$1"
  dest="$2"
  if [ ! -f "$src" ]; then
    echo "Missing binary in archive: $src" >&2
    exit 1
  fi
  cp "$src" "$dest"
  chmod +x "$dest"
}

cleanup_old_binary() {
  bin="$1"
  found=""
  OLD_IFS="$IFS"
  IFS=':'
  for dir in $PATH; do
    [ "$dir" = "$BIN_DIR" ] && continue
    [ -f "$dir/$bin" ] && found="$found $dir/$bin"
  done
  IFS="$OLD_IFS"

  for old in $found; do
    if [ "$old" -ef "${BIN_DIR}/${bin}" ] 2>/dev/null; then
      continue
    fi
    if [ "$FORCE" = "yes" ]; then
      rm -f "$old" && echo "  removed old: $old"
    else
      printf "Remove old installation at %s? [Y/n] " "$old"
      if tty -s 2>/dev/null; then
        read -r answer || answer=n
      else
        answer=n
        echo "non-interactive, keeping: $old" >&2
      fi
      case "$answer" in
        [Nn]*) ;;
        *) rm -f "$old" && echo "  removed" ;;
      esac
    fi
  done
}

install_bin "${TMP_DIR}/acari" "${BIN_DIR}/acari"
install_bin "${TMP_DIR}/headless_cleaner" "${BIN_DIR}/headless_cleaner"

echo "Installed binaries to: ${BIN_DIR}"

echo "Installed versions:"
"${BIN_DIR}/acari" --version || true
"${BIN_DIR}/headless_cleaner" --version || true

if ! printf '%s' ":$PATH:" | grep -q ":${BIN_DIR}:"; then
  echo ""
  echo "${BIN_DIR} is not in PATH. Add it for your shell:"
  echo "- bash/zsh: export PATH=\"${BIN_DIR}:\$PATH\""
  echo "- fish: set -Ux fish_user_paths ${BIN_DIR} \$fish_user_paths"
fi

cleanup_old_binary "acari"
cleanup_old_binary "headless_cleaner"

echo "Done."
