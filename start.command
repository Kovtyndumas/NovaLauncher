#!/bin/sh
set -eu
cd "$(dirname "$0")"
if command -v cargo >/dev/null 2>&1; then
  cargo run --release --locked
else
  export CARGO_HOME="$HOME/.local/share/nova-rust-tools/cargo"
  export RUSTUP_HOME="$HOME/.local/share/nova-rust-tools/rustup"
  "$CARGO_HOME/bin/cargo" run --release --locked
fi
