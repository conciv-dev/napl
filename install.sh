#!/bin/sh
set -eu

REPO="conciv-dev/napl"
INSTALL_DIR="${NAPL_INSTALL:-$HOME/.local/bin}"
VERSION="${NAPL_VERSION:-latest}"

info() {
  printf 'napl-install: %s\n' "$1"
}

die() {
  printf 'napl-install: error: %s\n' "$1" >&2
  exit 1
}

detect_os() {
  os="$(uname -s)"
  case "$os" in
    Darwin) printf 'apple-darwin' ;;
    Linux) printf 'unknown-linux-gnu' ;;
    *) die "unsupported operating system '$os'. On Windows install via npm: npm i -g napl-lang" ;;
  esac
}

detect_arch() {
  arch="$(uname -m)"
  case "$arch" in
    arm64 | aarch64) printf 'aarch64' ;;
    x86_64 | amd64) printf 'x86_64' ;;
    *) die "unsupported architecture '$arch'" ;;
  esac
}

download() {
  url="$1"
  out="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fSL --proto '=https' --tlsv1.2 "$url" -o "$out" || return 1
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$out" || return 1
  else
    die "neither curl nor wget is available"
  fi
}

main() {
  os="$(detect_os)"
  arch="$(detect_arch)"
  target="${arch}-${os}"
  asset="napl-${target}"

  if [ "$VERSION" = "latest" ]; then
    url="https://github.com/${REPO}/releases/latest/download/${asset}"
  else
    url="https://github.com/${REPO}/releases/download/${VERSION}/${asset}"
  fi

  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  info "downloading ${asset} (${VERSION})"
  if ! download "$url" "${tmp}/napl"; then
    die "download failed from ${url}. Install from source: cargo install --git https://github.com/${REPO} napl-cli"
  fi

  mkdir -p "$INSTALL_DIR"
  chmod +x "${tmp}/napl"
  mv "${tmp}/napl" "${INSTALL_DIR}/napl"
  info "installed napl to ${INSTALL_DIR}/napl"

  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
      info "run 'napl --help' to get started"
      ;;
    *)
      info "add ${INSTALL_DIR} to your PATH, e.g.:"
      # shellcheck disable=SC2016
      printf '  export PATH="%s:$PATH"\n' "$INSTALL_DIR"
      ;;
  esac
}

main
