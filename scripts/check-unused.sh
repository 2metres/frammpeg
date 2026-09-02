#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo-machete &> /dev/null; then
    echo "Error: cargo-machete is not installed."
    echo ""
    echo "Install it with:"
    echo "  cargo install cargo-machete"
    exit 1
fi

cargo machete
