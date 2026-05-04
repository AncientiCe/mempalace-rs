#!/usr/bin/env sh
set -eu

repo="${MEMPALACE_REPO:-AncientiCe/mempalace-rs}"
install_dir="${MEMPALACE_INSTALL_DIR:-$HOME/.local/bin}"
tmp_dir="$(mktemp -d)"

cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Linux) os_part="unknown-linux-gnu" ;;
    Darwin) os_part="apple-darwin" ;;
    *) echo "Unsupported OS: $os" >&2; exit 1 ;;
  esac
  case "$arch" in
    x86_64|amd64) arch_part="x86_64" ;;
    arm64|aarch64) arch_part="aarch64" ;;
    *) echo "Unsupported architecture: $arch" >&2; exit 1 ;;
  esac
  if [ "$os_part" = "unknown-linux-gnu" ] && [ "$arch_part" = "aarch64" ]; then
    echo "Linux ARM64 release binaries are not shipped in v1; build from source with cargo install --path . for now." >&2
    exit 1
  fi
  printf '%s-%s' "$arch_part" "$os_part"
}

checksum_verify() {
  file="$1"
  checksum_file="$2"
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$(dirname "$file")" && sha256sum -c "$(basename "$checksum_file")")
  else
    (cd "$(dirname "$file")" && shasum -a 256 -c "$(basename "$checksum_file")")
  fi
}

target="$(detect_target)"

if [ "${MEMPALACE_VERSION:-}" = "local" ]; then
  if [ -z "${MEMPALACE_LOCAL_ARCHIVE:-}" ]; then
    echo "MEMPALACE_LOCAL_ARCHIVE is required when MEMPALACE_VERSION=local" >&2
    exit 1
  fi
  archive="$MEMPALACE_LOCAL_ARCHIVE"
else
  if [ -n "${MEMPALACE_VERSION:-}" ]; then
    tag="$MEMPALACE_VERSION"
  else
    tag="$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
  fi
  version="${tag#v}"
  asset="mempalace-$version-$target.tar.gz"
  archive="$tmp_dir/$asset"
  checksum="$tmp_dir/mempalace-$target.sha256"
  curl -fL "https://github.com/$repo/releases/download/$tag/$asset" -o "$archive"
  curl -fL "https://github.com/$repo/releases/download/$tag/mempalace-$target.sha256" -o "$checksum"
  checksum_verify "$archive" "$checksum"
fi

mkdir -p "$install_dir"
tar -xzf "$archive" -C "$tmp_dir"
binary="$(find "$tmp_dir" -type f -name mempalace | head -n 1)"
if [ -z "$binary" ]; then
  echo "Archive did not contain a mempalace binary" >&2
  exit 1
fi
cp "$binary" "$install_dir/mempalace"
chmod +x "$install_dir/mempalace"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *)
    echo "Add MemPalace to PATH:"
    echo "  export PATH=\"$install_dir:\$PATH\""
    ;;
esac

"$install_dir/mempalace" install --all

echo "MemPalace installed."
echo "Next: mempalace init <project> && mempalace mine <project>"
echo "Restart Cursor, Codex, or Claude Code to load the MCP server."
