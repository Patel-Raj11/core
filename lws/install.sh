#!/usr/bin/env bash
set -euo pipefail

REPO="https://github.com/dawnlabsai/lws.git"
INSTALL_DIR="${LWS_INSTALL_DIR:-$HOME/.lws/bin}"
MIN_RUST="1.70.0"

info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
err()   { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

# --- Check / install Rust ---
install_rust() {
  if command -v rustup &>/dev/null; then
    info "Rust already installed ($(rustc --version))"
  elif command -v rustc &>/dev/null; then
    info "rustc found but no rustup — skipping Rust install"
  else
    info "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --quiet
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
}

# --- Ensure cargo is on PATH ---
ensure_cargo() {
  if ! command -v cargo &>/dev/null; then
    if [ -f "$HOME/.cargo/env" ]; then
      . "$HOME/.cargo/env"
    else
      err "cargo not found. Install Rust: https://rustup.rs"
    fi
  fi
}

# --- Build ---
build() {
  local src_dir="$1"
  info "Building lws workspace..."
  cd "$src_dir/lws"
  cargo build --workspace --release
  info "Build complete"
}

# --- Install binary (future: when lws-cli crate is added) ---
install_bin() {
  local src_dir="$1"
  info "Running tests to verify build..."
  cd "$src_dir/lws"
  cargo test --workspace --release --quiet
  info "All tests passed"
}

# --- Main ---
main() {
  info "LWS installer"
  echo

  install_rust
  ensure_cargo

  # Determine source directory
  if [ -f "lws/Cargo.toml" ]; then
    SRC_DIR="$(pwd)"
    info "Using local source: $SRC_DIR/lws"
  elif [ -f "Cargo.toml" ] && grep -q 'lws-core' Cargo.toml 2>/dev/null; then
    SRC_DIR="$(cd .. && pwd)"
    info "Using local source: $SRC_DIR/lws"
  else
    SRC_DIR="$(mktemp -d)"
    info "Cloning repository..."
    git clone --depth 1 "$REPO" "$SRC_DIR"
  fi

  build "$SRC_DIR"
  install_bin "$SRC_DIR"

  echo
  info "LWS installed successfully"
  info "Libraries built at: $SRC_DIR/lws/target/release"
}

main "$@"
