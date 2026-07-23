#!/bin/sh
# jenkins-cli installer — downloads the latest release binary for your platform.
# Usage: curl -fsSL https://raw.githubusercontent.com/MarcosT96/jenkins-cli/main/install.sh | sh
set -eu

REPO="MarcosT96/jenkins-cli"
BIN="jenkins"

# --- Detect OS ---
os="$(uname -s)"
case "$os" in
  Darwin) os_part="apple-darwin" ;;
  Linux)  os_part="unknown-linux-gnu" ;;
  *)
    echo "Error: unsupported OS '$os'. Prebuilt binaries exist only for macOS and Linux." >&2
    echo "Install from source instead: cargo install --git https://github.com/$REPO" >&2
    exit 1
    ;;
esac

# --- Detect arch ---
arch="$(uname -m)"
case "$arch" in
  arm64|aarch64) arch_part="aarch64" ;;
  x86_64|amd64)  arch_part="x86_64" ;;
  *)
    echo "Error: unsupported architecture '$arch'. Prebuilt binaries exist only for x86_64 and aarch64/arm64." >&2
    echo "Install from source instead: cargo install --git https://github.com/$REPO" >&2
    exit 1
    ;;
esac

target="${arch_part}-${os_part}"
asset="${BIN}-${target}"
url="https://github.com/${REPO}/releases/latest/download/${asset}"

echo "Detected platform: ${target}"
echo "Downloading ${asset} ..."

tmp="$(mktemp)"
if ! curl -fsSL "$url" -o "$tmp"; then
  echo "Error: download failed from $url" >&2
  echo "No prebuilt binary for '${target}' (the built targets are:" >&2
  echo "  aarch64-apple-darwin, x86_64-apple-darwin," >&2
  echo "  x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu)." >&2
  rm -f "$tmp"
  exit 1
fi

chmod +x "$tmp"

# --- Install location ---
# Install to ~/.local/bin, printing a PATH reminder. Used as the fallback
# whenever a system-wide install isn't possible.
user_install() {
  dest_dir="${HOME}/.local/bin"
  mkdir -p "$dest_dir"
  mv "$tmp" "${dest_dir}/${BIN}"
  echo "Installed ${BIN} to ${dest_dir}/${BIN}"
  echo "Note: make sure '${dest_dir}' is on your PATH:"
  echo '  export PATH="$HOME/.local/bin:$PATH"'
  echo "Run 'jenkins --help' to get started."
}

# Prefer /usr/local/bin, then sudo, then a user-local install. Each step is
# non-fatal: if it can't complete, we fall through to the next option rather
# than aborting under `set -e`.
dest_dir="/usr/local/bin"
if [ -w "$dest_dir" ] || [ "$(id -u)" -eq 0 ]; then
  mv "$tmp" "${dest_dir}/${BIN}"
  echo "Installed ${BIN} to ${dest_dir}/${BIN}"
  echo "Run 'jenkins --help' to get started."
elif command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
  # Passwordless sudo available — use it.
  sudo mv "$tmp" "${dest_dir}/${BIN}"
  echo "Installed ${BIN} to ${dest_dir}/${BIN}"
  echo "Run 'jenkins --help' to get started."
elif command -v sudo >/dev/null 2>&1 && sudo mv "$tmp" "${dest_dir}/${BIN}" 2>/dev/null; then
  # Interactive sudo succeeded.
  echo "Installed ${BIN} to ${dest_dir}/${BIN}"
  echo "Run 'jenkins --help' to get started."
else
  # No writable system dir and no usable sudo — install for the current user.
  user_install
fi
