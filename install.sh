#!/usr/bin/env bash
set -e

echo "Building Slate..."
cargo build --release

INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "$INSTALL_DIR"
cp target/release/slate "$INSTALL_DIR/slate"

echo "Installed to $INSTALL_DIR/slate\n"
