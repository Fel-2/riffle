#!/bin/sh
set -eu

REPO="Fel-2/riffle"
BIN="riffle"
VERSION="${VERSION:-}"
PREFIX="${PREFIX:-$HOME/.local}"
DEST="$PREFIX/bin"

fail() {
    printf 'riffle: %s\n' "$*" >&2
    exit 1
}

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"

[ "$(uname -s)" = "Linux" ] || fail "only Linux is supported"

case "$(uname -m)" in
    x86_64 | amd64) target="x86_64-unknown-linux-musl" ;;
    aarch64 | arm64) target="aarch64-unknown-linux-gnu" ;;
    *) fail "unsupported architecture: $(uname -m)" ;;
esac

if [ -z "$VERSION" ]; then
    VERSION=$(
        curl -fsSL --max-time 10 \
            -H 'User-Agent: riffle-installer' \
            -H 'Accept: application/vnd.github+json' \
            "https://api.github.com/repos/$REPO/releases/latest" |
            sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
            head -n 1
    )
    [ -n "$VERSION" ] || fail "could not determine the latest version"
fi

case "$VERSION" in
    v*) ;;
    *) VERSION="v$VERSION" ;;
esac

asset="$BIN-$VERSION-$target.tar.gz"
base="https://github.com/$REPO/releases/download/$VERSION"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/riffle.XXXXXX")
trap 'rm -rf "$tmp"' EXIT INT TERM

printf 'Downloading %s %s for %s...\n' "$BIN" "$VERSION" "$target"
curl -fsSL --max-time 120 "$base/$asset" -o "$tmp/$asset" ||
    fail "download failed"
curl -fsSL --max-time 30 "$base/SHA256SUMS" -o "$tmp/SHA256SUMS" ||
    fail "checksum download failed"

line=$(grep "  $asset\$" "$tmp/SHA256SUMS" || true)
[ -n "$line" ] || fail "checksum for $asset not found"
if command -v sha256sum >/dev/null 2>&1; then
    printf '%s\n' "$line" | (cd "$tmp" && sha256sum -c -) ||
        fail "checksum verification failed"
elif command -v shasum >/dev/null 2>&1; then
    printf '%s\n' "$line" | (cd "$tmp" && shasum -a 256 -c -) ||
        fail "checksum verification failed"
else
    printf 'warning: sha256sum/shasum not found, skipping checksum\n' >&2
fi

tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$DEST"
if command -v install >/dev/null 2>&1; then
    install -m 755 "$tmp/$BIN-$VERSION-$target/$BIN" "$DEST/$BIN"
else
    cp "$tmp/$BIN-$VERSION-$target/$BIN" "$DEST/$BIN"
    chmod 755 "$DEST/$BIN"
fi

printf 'Installed %s %s to %s\n' "$BIN" "$VERSION" "$DEST/$BIN"
case ":$PATH:" in
    *":$DEST:"*) ;;
    *) printf 'Add %s to PATH to run %s.\n' "$DEST" "$BIN" ;;
esac
