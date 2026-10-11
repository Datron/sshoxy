#!/bin/sh
# Install the latest sshoxy release from GitHub into a bin directory.
#
# Usage:
#   sh install.sh [--version <tag>] [--bin-dir <dir>] [--repo <owner/repo>]
#
# Examples:
#   # latest release -> $HOME/.local/bin
#   curl -fsSL https://raw.githubusercontent.com/Datron/sshoxy/master/install.sh | sh
#
#   # pinned version, custom bin dir
#   sh install.sh --version v0.1.0 --bin-dir "$HOME/bin"
#
# Env overrides (flags win over env):
#   SSHOXY_VERSION   release tag, e.g. "v0.1.0" (default: latest)
#   SSHOXY_BIN_DIR   install directory (default: $HOME/.local/bin)
#   SSHOXY_REPO      GitHub repo (default: Datron/sshoxy)
#   SSHOXY_BASE_URL  release host (default: https://github.com)
set -eu

REPO="${SSHOXY_REPO:-Datron/sshoxy}"
BASE_URL="${SSHOXY_BASE_URL:-https://github.com}"
VERSION="${SSHOXY_VERSION:-latest}"
BIN_DIR="${SSHOXY_BIN_DIR:-$HOME/.local/bin}"

usage() {
  cat <<EOF
Usage: install.sh [options]

Options:
  --version <tag>   Release tag to install (e.g. v0.1.0). Default: latest.
  --bin-dir <dir>   Directory to install the sshoxy binary into.
                    Default: \$HOME/.local/bin (or \$SSHOXY_BIN_DIR).
  --repo <o/r>      GitHub repository. Default: Datron/sshoxy (or \$SSHOXY_REPO).
  -h, --help        Show this help and exit.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version) VERSION="${2:?--version needs a value}"; shift 2 ;;
    --version=*) VERSION="${1#--version=}"; shift ;;
    --bin-dir) BIN_DIR="${2:?--bin-dir needs a value}"; shift 2 ;;
    --bin-dir=*) BIN_DIR="${1#--bin-dir=}"; shift ;;
    --repo) REPO="${2:?--repo needs a value}"; shift 2 ;;
    --repo=*) REPO="${1#--repo=}"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "error: unknown argument: $1" >&2; usage >&2; exit 1 ;;
  esac
done

# Map uname output to a release target triple. Prints triple on stdout.
detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) echo "error: unsupported architecture: $arch (Linux)" >&2; return 1 ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        arm64|aarch64) echo "aarch64-apple-darwin" ;;
        x86_64|amd64) echo "x86_64-apple-darwin" ;;
        *) echo "error: unsupported architecture: $arch (macOS)" >&2; return 1 ;;
      esac
      ;;
    *) echo "error: unsupported OS: $os (only Linux and macOS are released)" >&2; return 1 ;;
  esac
}

download() {
  url="$1"
  dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 -o "$dest" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$dest" "$url"
  else
    echo "error: need curl or wget to download releases" >&2
    return 1
  fi
}

extract_binary() {
  zip="$1"
  dest_dir="$2"
  if command -v unzip >/dev/null 2>&1; then
    unzip -o -j "$zip" sshoxy -d "$dest_dir"
  elif command -v python3 >/dev/null 2>&1; then
    python3 -c "import sys,zipfile; z=zipfile.ZipFile(sys.argv[1]); z.extract('sshoxy', sys.argv[2])" "$zip" "$dest_dir"
  else
    echo "error: need unzip or python3 to extract the release zip" >&2
    return 1
  fi
}

TARGET="$(detect_target)"
ASSET="sshoxy-${TARGET}.zip"

if [ "$VERSION" = "latest" ]; then
  URL="${BASE_URL}/${REPO}/releases/latest/download/${ASSET}"
else
  case "$VERSION" in
    v*) ;;
    *) VERSION="v${VERSION}" ;;
  esac
  URL="${BASE_URL}/${REPO}/releases/download/${VERSION}/${ASSET}"
fi

echo "sshoxy setup"
echo "  repo:    $REPO"
echo "  version: $VERSION"
echo "  target:  $TARGET"
echo "  asset:   $ASSET"
echo "  bin:     $BIN_DIR"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

echo "Downloading $URL ..."
download "$URL" "$TMP_DIR/$ASSET" || {
  echo "error: download failed." >&2
  if [ "$VERSION" = "latest" ]; then
    echo "Is there a published release at https://github.com/${REPO}/releases ?" >&2
  else
    echo "Does $VERSION with asset $ASSET exist at https://github.com/${REPO}/releases/tag/$VERSION ?" >&2
  fi
  exit 1
}

mkdir -p "$BIN_DIR"
echo "Installing sshoxy to $BIN_DIR/sshoxy ..."
extract_binary "$TMP_DIR/$ASSET" "$BIN_DIR"
chmod +x "$BIN_DIR/sshoxy"

if "$BIN_DIR/sshoxy" --version >/dev/null 2>&1; then
  echo "Installed: $("$BIN_DIR/sshoxy" --version)"
else
  echo "warning: installed but '$BIN_DIR/sshoxy --version' failed to run" >&2
fi

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "Note: $BIN_DIR is not on your PATH. Add it, e.g.: export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac
